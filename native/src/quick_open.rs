//! Quick Open (Ctrl+P): cerca un file del progetto per nome con la ricerca fuzzy della versione Tauri.
//! Frecce per scegliere, Invio o doppio clic per aprire, Esc per chiudere.
use std::cell::RefCell;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_DOWN, VK_ESCAPE, VK_NEXT, VK_PRIOR, VK_RETURN, VK_UP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::find::window_text;
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorQuickOpen";
const WIDTH: i32 = 620;
const HEIGHT: i32 = 420;
const LIMIT: usize = 200;
const NAME_BONUS: f64 = 10.0;
const CONSECUTIVE_BONUS: f64 = 5.0;
const BOUNDARY_BONUS: f64 = 4.0;
const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_SETCURSEL: u32 = 0x0186;
const LB_GETCURSEL: u32 = 0x0188;
const LB_GETCOUNT: u32 = 0x018B;
const LBN_DBLCLK: u32 = 2;
const EN_CHANGE: u32 = 0x0300;

/// Lettere della ricerca in ordine nel testo: più punti se consecutive o a inizio parola; None se non ci sono tutte.
fn subsequence_score(query: &[char], target: &[char], offset: usize) -> Option<f64> {
    let (mut score, mut from, mut previous) = (0.0, 0usize, None::<usize>);
    for &wanted in query {
        let found = from + target[from..].iter().position(|&ch| ch == wanted)?;
        if previous.is_some_and(|previous| found == previous + 1) {
            score += CONSECUTIVE_BONUS;
        }
        if found == 0 || matches!(target[found - 1], '/' | '\\' | '.' | '-' | '_' | ' ') {
            score += BOUNDARY_BONUS;
        }
        score -= (found - from).min(10) as f64 * 0.1;
        previous = Some(found);
        from = found + 1;
    }
    Some(score - (target.len() + offset) as f64 * 0.01)
}

pub fn fuzzy_score(query: &str, path: &str) -> Option<f64> {
    let query: Vec<char> = query.to_lowercase().chars().filter(|ch| !ch.is_whitespace()).collect();
    if query.is_empty() {
        return Some(0.0);
    }
    let target: Vec<char> = path.to_lowercase().chars().collect();
    let name_start = target.iter().rposition(|&ch| ch == '/' || ch == '\\').map_or(0, |index| index + 1);
    subsequence_score(&query, &target[name_start..], name_start).map(|score| score + NAME_BONUS).or_else(|| subsequence_score(&query, &target, 0))
}

/// I percorsi migliori per la ricerca, al massimo `limit`.
pub fn fuzzy_filter<'a>(query: &str, paths: &'a [String], limit: usize) -> Vec<&'a String> {
    let mut scored: Vec<(f64, &String)> = paths.iter().filter_map(|path| fuzzy_score(query, path).map(|score| (score, path))).collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().take(limit).map(|(_, path)| path).collect()
}

struct State {
    palette: Palette,
    brush: HBRUSH,
    files: Vec<String>,
    shown: Vec<String>,
    list: HWND,
    changed: bool,
    done: Option<bool>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

unsafe extern "system" fn quick_open_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let handled = STATE.with(|state| {
        let mut state = state.try_borrow_mut().ok()?;
        let state = state.as_mut()?;
        match message {
            WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                SetTextColor(wparam as HDC, state.palette.fg);
                SetBkColor(wparam as HDC, state.palette.editor);
                Some(state.brush as LRESULT)
            }
            WM_COMMAND => {
                let code = ((wparam >> 16) & 0xFFFF) as u32;
                if code == EN_CHANGE {
                    state.changed = true;
                } else if lparam as HWND == state.list && code == LBN_DBLCLK {
                    state.done = Some(true);
                }
                Some(0)
            }
            WM_CLOSE => {
                state.done = Some(false);
                Some(0)
            }
            _ => None,
        }
    });
    handled.unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
}

fn refill(query: &str) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else { return };
        state.shown = fuzzy_filter(query, &state.files, LIMIT).into_iter().cloned().collect();
        // SAFETY: lista valida; le stringhe vivono per la chiamata.
        unsafe {
            SendMessageW(state.list, LB_RESETCONTENT, 0, 0);
            for path in &state.shown {
                let name = Path::new(path).file_name().map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());
                let parent = Path::new(path).parent().map(|dir| dir.display().to_string()).filter(|dir| !dir.is_empty());
                let label = wide(&parent.map_or_else(|| name.clone(), |dir| format!("{name}    {dir}")));
                SendMessageW(state.list, LB_ADDSTRING, 0, label.as_ptr() as isize);
            }
            SendMessageW(state.list, LB_SETCURSEL, 0, 0);
        }
    });
}

/// Mostra Quick Open sui file di `root`; il file scelto, se c'è.
pub fn pick(owner: HWND, palette: &Palette, font: HFONT, root: &Path, files: Vec<String>) -> Option<PathBuf> {
    let relative: Vec<String> = files.iter().map(|file| Path::new(file).strip_prefix(root).map_or_else(|_| file.clone(), |rest| rest.to_string_lossy().into_owned())).collect();
    let class_name = wide(CLASS_NAME);
    let title = wide("Apri file");
    // SAFETY: finestra modale creata, usata e distrutta qui; il proprietario è disabilitato per tutta la durata.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSEXW { cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(quick_open_proc), hInstance: instance, hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW), hbrBackground: std::ptr::null_mut(), lpszClassName: class_name.as_ptr(), ..std::mem::zeroed() };
        RegisterClassExW(&class);
        let scale = |value: i32| value * GetDpiForWindow(owner) as i32 / 96;
        let mut owner_rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(owner, &mut owner_rect);
        let (width, height) = (scale(WIDTH), scale(HEIGHT));
        let left = owner_rect.left + (owner_rect.right - owner_rect.left - width) / 2;
        let hwnd = CreateWindowExW(WS_EX_DLGMODALFRAME, class_name.as_ptr(), title.as_ptr(), WS_POPUP | WS_CAPTION | WS_SYSMENU, left, owner_rect.top + scale(80), width, height, owner, std::ptr::null_mut(), instance, std::ptr::null());
        let dark: i32 = palette.dark.into();
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, (&dark as *const i32).cast(), 4);
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(hwnd, &mut client);
        let (edit_class, list_class) = (wide("EDIT"), wide("LISTBOX"));
        let edit = CreateWindowExW(0, edit_class.as_ptr(), std::ptr::null(), WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL as u32, scale(8), scale(8), client.right - scale(16), scale(28), hwnd, std::ptr::null_mut(), instance, std::ptr::null());
        let list_style = WS_CHILD | WS_VISIBLE | WS_VSCROLL | (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32;
        let list = CreateWindowExW(0, list_class.as_ptr(), std::ptr::null(), list_style, scale(8), scale(44), client.right - scale(16), client.bottom - scale(52), hwnd, std::ptr::null_mut(), instance, std::ptr::null());
        if palette.dark {
            let theme = wide("DarkMode_Explorer");
            SetWindowTheme(list, theme.as_ptr(), std::ptr::null());
        }
        for control in [edit, list] {
            SendMessageW(control, WM_SETFONT, font as usize, 0);
        }
        let cue = wide("Nome del file (anche lettere sparse, es. apprs per app.rs)");
        SendMessageW(edit, 0x1501, 1, cue.as_ptr() as isize);
        STATE.with(|state| *state.borrow_mut() = Some(State { palette: *palette, brush: CreateSolidBrush(palette.editor), files: relative, shown: Vec::new(), list, changed: false, done: None }));
        refill("");

        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(edit);
        let mut msg: MSG = std::mem::zeroed();
        let accepted = loop {
            if let Some(done) = STATE.with(|state| state.borrow().as_ref().and_then(|state| state.done)) {
                break done;
            }
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                // WM_QUIT arrivato durante il dialogo: va rimesso in coda per il ciclo principale.
                PostQuitMessage(0);
                break false;
            }
            if msg.message == WM_KEYDOWN {
                let key = msg.wParam as u16;
                let count = SendMessageW(list, LB_GETCOUNT, 0, 0).max(1);
                let current = SendMessageW(list, LB_GETCURSEL, 0, 0).max(0);
                let step = match key {
                    VK_RETURN => break true,
                    VK_ESCAPE => break false,
                    VK_DOWN => 1,
                    VK_UP => -1,
                    VK_NEXT => 10,
                    VK_PRIOR => -10,
                    _ => 0,
                };
                if step != 0 {
                    SendMessageW(list, LB_SETCURSEL, (current + step).clamp(0, count - 1) as usize, 0);
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if STATE.with(|state| state.borrow_mut().as_mut().is_some_and(|state| std::mem::take(&mut state.changed))) {
                refill(&window_text(edit));
            }
        };
        let selected = SendMessageW(list, LB_GETCURSEL, 0, 0);
        let state = STATE.with(|state| state.borrow_mut().take());
        EnableWindow(owner, 1);
        DestroyWindow(hwnd);
        SetFocus(owner);
        let state = state?;
        DeleteObject(state.brush);
        let chosen = usize::try_from(selected).ok().and_then(|index| state.shown.get(index)).filter(|_| accepted)?;
        Some(root.join(chosen))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_names_before_folders() {
        let paths: Vec<String> = ["src/app.rs", "native/src/panel.rs", "apps/readme.md", "src/plugins/mod.rs"].map(String::from).to_vec();
        assert_eq!(fuzzy_filter("apprs", &paths, 10)[0], "src/app.rs");
        assert_eq!(fuzzy_filter("pnl", &paths, 10), vec!["native/src/panel.rs"]);
        assert!(fuzzy_score("xyz", "src/app.rs").is_none());
        assert_eq!(fuzzy_filter("", &paths, 2).len(), 2);
    }
}

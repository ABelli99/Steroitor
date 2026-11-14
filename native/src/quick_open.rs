//! Quick Open (Ctrl+P), come quello della versione Tauri: riquadro al 10% dall'alto, campo "Cerca file per nome…",
//! al più 50 risultati con il nome e la cartella attenuata. Frecce e Pag su/giù per scegliere, Invio o clic per aprire,
//! Esc per chiudere. La ricerca fuzzy è la stessa della versione Tauri.
use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_DOWN, VK_ESCAPE, VK_NEXT, VK_PRIOR, VK_RETURN, VK_UP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::find::window_text;
use crate::list::{decode_event, List, ListEvent, RowPaint, WM_LIST_EVENT, WM_LIST_PAINT};
use crate::paint::{fill, frame, text, text_width};
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorQuickOpen";
const WIDTH: i32 = 600;
const INPUT: i32 = 36;
const ROW: i32 = 24;
const LIMIT: usize = 50;
const NAME_BONUS: f64 = 10.0;
const CONSECUTIVE_BONUS: f64 = 5.0;
const BOUNDARY_BONUS: f64 = 4.0;
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

/// Controlli del riquadro, creati alla prima apertura e riusati.
struct Ui {
    hwnd: HWND,
    edit: HWND,
    list: List,
}

struct State {
    palette: Palette,
    brush: HBRUSH,
    font: HFONT,
    files: Vec<String>,
    shown: Vec<String>,
    changed: bool,
    done: Option<bool>,
    max_rows: usize,
    width: i32,
}

thread_local! {
    static UI: OnceCell<&'static Ui> = const { OnceCell::new() };
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn ui() -> Option<&'static Ui> {
    UI.with(|cell| cell.get().copied())
}

unsafe fn paint_row(row: &RowPaint) {
    STATE.with(|state| {
        let state = state.borrow();
        let Some(state) = state.as_ref() else { return };
        let Some(path) = state.shown.get(row.index) else { return };
        let palette = &state.palette;
        let dc = row.dc;
        SelectObject(dc, state.font);
        SetBkMode(dc, TRANSPARENT as i32);
        fill(dc, &row.rect, if row.selected { palette.selection } else { palette.panel });
        let s = |value: i32| value * GetDpiForWindow(row.list) as i32 / 96;
        let name = Path::new(path).file_name().map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());
        let dir = Path::new(path).parent().map(|dir| dir.display().to_string()).unwrap_or_default();
        let left = row.rect.left + s(12);
        let name_right = (left + text_width(dc, &name)).min(row.rect.right - s(12));
        text(dc, &name, &RECT { left, right: name_right, ..row.rect }, palette.fg, DT_LEFT | DT_END_ELLIPSIS);
        text(dc, &dir, &RECT { left: name_right + s(8), right: row.rect.right - s(12), ..row.rect }, palette.muted, DT_LEFT | DT_PATH_ELLIPSIS);
    });
}

unsafe extern "system" fn quick_open_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_LIST_PAINT => {
            paint_row(&*(lparam as *const RowPaint));
            return 0;
        }
        WM_LIST_EVENT => {
            // Clic o doppio clic su un risultato: si apre (le frecce del campo mandano Toggle, ignorato).
            let (event, _) = decode_event(lparam);
            if matches!(event, ListEvent::Select | ListEvent::Activate) {
                STATE.with(|state| {
                    if let Some(state) = state.borrow_mut().as_mut() {
                        state.done = Some(true);
                    }
                });
            }
            return 0;
        }
        _ => {}
    }
    let handled = STATE.with(|state| {
        let mut state = state.try_borrow_mut().ok()?;
        let state = state.as_mut()?;
        match message {
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut paint);
                let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                GetClientRect(hwnd, &mut client);
                let s = |value: i32| value * GetDpiForWindow(hwnd) as i32 / 96;
                let palette = &state.palette;
                fill(dc, &client, palette.panel);
                let input = RECT { bottom: s(INPUT), ..client };
                fill(dc, &input, palette.editor);
                fill(dc, &RECT { top: input.bottom - 1, ..input }, palette.border);
                if state.shown.is_empty() {
                    let previous = SelectObject(dc, state.font);
                    SetBkMode(dc, TRANSPARENT as i32);
                    text(dc, "Nessun risultato", &RECT { left: s(12), top: input.bottom, right: client.right - s(12), bottom: input.bottom + s(ROW) }, palette.muted, DT_LEFT);
                    SelectObject(dc, previous);
                }
                frame(dc, &client, palette.border);
                EndPaint(hwnd, &paint);
                Some(0)
            }
            WM_ERASEBKGND => Some(1),
            WM_CTLCOLOREDIT => {
                SetTextColor(wparam as HDC, state.palette.fg);
                SetBkColor(wparam as HDC, state.palette.editor);
                Some(state.brush as LRESULT)
            }
            WM_COMMAND => {
                if ((wparam >> 16) & 0xFFFF) as u32 == EN_CHANGE {
                    state.changed = true;
                }
                Some(0)
            }
            WM_ACTIVATE if (wparam & 0xFFFF) as u32 == WA_INACTIVE => {
                // Un clic fuori chiude, come lo sfondo della versione Tauri.
                state.done.get_or_insert(false);
                Some(0)
            }
            _ => None,
        }
    });
    handled.unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
}

/// Risultati per la ricerca; il riquadro si allunga fino a metà della finestra.
fn refill(ui: &Ui, query: &str) {
    let (count, max_rows, width) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else { return (0, 1, 0) };
        state.shown = fuzzy_filter(query, &state.files, LIMIT).into_iter().cloned().collect();
        (state.shown.len(), state.max_rows, state.width)
    });
    // SAFETY: finestre valide del riquadro.
    unsafe {
        let s = |value: i32| value * GetDpiForWindow(ui.hwnd) as i32 / 96;
        let rows = count.clamp(1, max_rows) as i32;
        let height = s(INPUT) + rows * s(ROW) + s(6);
        SetWindowPos(ui.hwnd, std::ptr::null_mut(), 0, 0, width, height, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
        ShowWindow(ui.list.hwnd, if count == 0 { SW_HIDE } else { SW_SHOW });
        MoveWindow(ui.list.hwnd, 1, s(INPUT), width - 2, rows * s(ROW) + s(4), 1);
        ui.list.set_rows(count);
        if count > 0 {
            ui.list.select(0, ListEvent::Toggle);
        }
        InvalidateRect(ui.hwnd, std::ptr::null(), 1);
    }
}

fn create_ui(owner: HWND, palette: &Palette) -> &'static Ui {
    if let Some(ui) = ui() {
        return ui;
    }
    let class_name = wide(CLASS_NAME);
    // SAFETY: classe e finestre create una volta e tenute per tutta la vita dell'app.
    let ui: &'static Ui = unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSEXW { cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(quick_open_proc), hInstance: instance, hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW), lpszClassName: class_name.as_ptr(), ..std::mem::zeroed() };
        RegisterClassExW(&class);
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), std::ptr::null(), WS_POPUP | WS_CLIPCHILDREN, 0, 0, 0, 0, owner, std::ptr::null_mut(), instance, std::ptr::null());
        let edit_class = wide("EDIT");
        let edit = CreateWindowExW(0, edit_class.as_ptr(), std::ptr::null(), WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32, 0, 0, 0, 0, hwnd, std::ptr::null_mut(), instance, std::ptr::null());
        Box::leak(Box::new(Ui { hwnd, edit, list: List::create(hwnd, instance, palette.panel, palette.dark) }))
    };
    ui.list.attach();
    UI.with(|cell| {
        let _ = cell.set(ui);
    });
    ui
}

/// Mostra Quick Open sui file di `root`; il file scelto, se c'è.
pub fn pick(owner: HWND, palette: &Palette, font: HFONT, root: &Path, files: Vec<String>) -> Option<PathBuf> {
    let relative: Vec<String> = files.iter().map(|file| Path::new(file).strip_prefix(root).map_or_else(|_| file.clone(), |rest| rest.to_string_lossy().into_owned())).collect();
    let ui = create_ui(owner, palette);
    // SAFETY: riquadro modale mostrato e nascosto qui; il proprietario è disabilitato per tutta la durata.
    unsafe {
        let s = |value: i32| value * GetDpiForWindow(owner) as i32 / 96;
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(owner, &mut client);
        let mut origin = POINT { x: 0, y: 0 };
        ClientToScreen(owner, &mut origin);
        let width = s(WIDTH).min(client.right - s(32)).max(s(300));
        let max_rows = ((client.bottom / 2) / s(ROW)).max(3) as usize;
        SetWindowPos(ui.hwnd, std::ptr::null_mut(), origin.x + (client.right - width) / 2, origin.y + client.bottom / 10, width, s(INPUT), SWP_NOZORDER | SWP_NOACTIVATE);
        SendMessageW(ui.edit, WM_SETFONT, font as usize, 0);
        MoveWindow(ui.edit, s(12), s(9), width - s(24), s(20), 1);
        let empty = wide("");
        SetWindowTextW(ui.edit, empty.as_ptr());
        let cue = wide("Cerca file per nome…");
        SendMessageW(ui.edit, 0x1501, 1, cue.as_ptr() as isize);
        ui.list.set_row_height(s(ROW));
        STATE.with(|state| *state.borrow_mut() = Some(State { palette: *palette, brush: CreateSolidBrush(palette.editor), font, files: relative, shown: Vec::new(), changed: false, done: None, max_rows, width }));
        refill(ui, "");

        EnableWindow(owner, 0);
        ShowWindow(ui.hwnd, SW_SHOW);
        SetFocus(ui.edit);
        let mut msg: MSG = std::mem::zeroed();
        let accepted = loop {
            if let Some(done) = STATE.with(|state| state.borrow().as_ref().and_then(|state| state.done)) {
                break done;
            }
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                // WM_QUIT arrivato durante il riquadro: va rimesso in coda per il ciclo principale.
                PostQuitMessage(0);
                break false;
            }
            if msg.message == WM_KEYDOWN {
                let key = msg.wParam as u16;
                let rows = STATE.with(|state| state.borrow().as_ref().map_or(0, |state| state.shown.len()));
                let current = ui.list.selected().unwrap_or(0) as isize;
                let step = match key {
                    VK_RETURN => break rows > 0,
                    VK_ESCAPE => break false,
                    VK_DOWN => 1,
                    VK_UP => -1,
                    VK_NEXT => 10,
                    VK_PRIOR => -10,
                    _ => 0,
                };
                if step != 0 {
                    if rows > 0 {
                        ui.list.select((current + step).clamp(0, rows as isize - 1) as usize, ListEvent::Toggle);
                    }
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if STATE.with(|state| state.borrow_mut().as_mut().is_some_and(|state| std::mem::take(&mut state.changed))) {
                refill(ui, &window_text(ui.edit));
            }
        };
        let selected = ui.list.selected();
        let state = STATE.with(|state| state.borrow_mut().take());
        EnableWindow(owner, 1);
        ShowWindow(ui.hwnd, SW_HIDE);
        SetFocus(owner);
        let state = state?;
        DeleteObject(state.brush);
        let chosen = selected.and_then(|index| state.shown.get(index)).filter(|_| accepted)?;
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

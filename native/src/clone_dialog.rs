//! Dialogo "Clona repository" come quello della versione Tauri: URL, cartella di destinazione (proposta da sola come
//! {ultima cartella o home}\{nome del repository} finché non la si cambia a mano), "Sfoglia…", avviso sulla cartella,
//! Annulla e Clona (attivo solo con URL e cartella validi). Invio clona, Esc annulla.
use std::cell::RefCell;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus, VK_ESCAPE, VK_RETURN, VK_TAB};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::{pick_folder, wide};
use crate::find::window_text;
use crate::git::repo_name;
use crate::paint::{fill, frame, text, text_width};
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorClone";
const WIDTH: i32 = 620;
const HEIGHT: i32 = 250;
const EN_CHANGE: u32 = 0x0300;

#[derive(Clone, Copy, PartialEq)]
enum Button {
    Browse,
    Cancel,
    Clone,
}

/// Stato della cartella scelta, mostrato sotto il campo; gli ultimi due impediscono il clone.
#[derive(Debug, PartialEq)]
enum Target {
    Missing,
    Empty,
    NotEmpty,
    File,
}

fn target_state(path: &Path) -> Target {
    if path.is_file() {
        return Target::File;
    }
    match std::fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                Target::NotEmpty
            } else {
                Target::Empty
            }
        }
        Err(_) => Target::Missing,
    }
}

struct State {
    palette: Palette,
    brush: HBRUSH,
    url: HWND,
    dir: HWND,
    parent: PathBuf,
    /// La cartella l'ha scritta l'utente: non la si propone più dall'URL.
    touched: bool,
    /// Il testo della cartella lo sta cambiando il dialogo (EN_CHANGE da ignorare).
    filling: bool,
    buttons: Vec<(RECT, Button)>,
    done: Option<bool>,
    browse: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn valid(url: &str, dir: &str) -> bool {
    repo_name(url).is_some() && !dir.trim().is_empty() && matches!(target_state(Path::new(dir.trim())), Target::Missing | Target::Empty)
}

unsafe extern "system" fn clone_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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
                frame(dc, &client, palette.border);
                let previous = SelectObject(dc, SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT);
                SetBkMode(dc, TRANSPARENT as i32);
                let row = |top: i32| RECT { left: s(16), top: s(top), right: client.right - s(16), bottom: s(top + 20) };
                text(dc, "Clona repository", &row(12), palette.fg, DT_LEFT);
                text(dc, "URL", &row(40), palette.muted, DT_LEFT);
                text(dc, "Directory", &row(96), palette.muted, DT_LEFT);
                for edit in [state.url, state.dir] {
                    let mut field = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    GetWindowRect(edit, &mut field);
                    let mut origin = POINT { x: field.left, y: field.top };
                    ScreenToClient(hwnd, &mut origin);
                    let border = RECT { left: origin.x - s(5), top: origin.y - s(4), right: origin.x + field.right - field.left + s(5), bottom: origin.y + field.bottom - field.top + s(4) };
                    fill(dc, &border, palette.editor);
                    frame(dc, &border, if GetFocus() == edit { palette.accent } else { palette.border });
                }
                let (url, dir) = (window_text(state.url), window_text(state.dir));
                let hint = if dir.trim().is_empty() {
                    None
                } else {
                    Some(match target_state(Path::new(dir.trim())) {
                        Target::Missing => ("La cartella verrà creata.", palette.muted),
                        Target::Empty => ("La cartella esiste ed è vuota.", palette.muted),
                        Target::NotEmpty => ("La cartella esiste già e non è vuota: scegline un'altra.", palette.danger),
                        Target::File => ("Esiste già un file con questo nome.", palette.danger),
                    })
                };
                if let Some((hint, color)) = hint {
                    text(dc, hint, &row(150), color, DT_LEFT | DT_END_ELLIPSIS);
                }
                let mut buttons = Vec::new();
                let browse_label = "Sfoglia…";
                let browse = RECT { left: client.right - s(16) - text_width(dc, browse_label) - s(24), top: s(118), right: client.right - s(16), bottom: s(146) };
                frame(dc, &browse, palette.border);
                text(dc, browse_label, &browse, palette.fg, DT_CENTER);
                buttons.push((browse, Button::Browse));
                let enabled = valid(&url, &dir);
                let mut right = client.right - s(16);
                for (label, button) in [("Clona", Button::Clone), ("Annulla", Button::Cancel)] {
                    let area = RECT { left: right - text_width(dc, label) - s(24), top: client.bottom - s(44), right, bottom: client.bottom - s(16) };
                    if button == Button::Clone && enabled {
                        fill(dc, &area, palette.accent);
                        text(dc, label, &area, 0xFFFFFF, DT_CENTER);
                    } else {
                        frame(dc, &area, palette.border);
                        text(dc, label, &area, if button == Button::Clone { palette.muted } else { palette.fg }, DT_CENTER);
                    }
                    if button != Button::Clone || enabled {
                        buttons.push((area, button));
                    }
                    right = area.left - s(8);
                }
                state.buttons = buttons;
                SelectObject(dc, previous);
                EndPaint(hwnd, &paint);
                Some(0)
            }
            WM_CTLCOLOREDIT => {
                SetTextColor(wparam as HDC, state.palette.fg);
                SetBkColor(wparam as HDC, state.palette.editor);
                Some(state.brush as LRESULT)
            }
            WM_COMMAND if ((wparam >> 16) & 0xFFFF) as u32 == EN_CHANGE => {
                let source = lparam as HWND;
                if source == state.dir && !state.filling {
                    state.touched = true;
                }
                if source == state.url && !state.touched {
                    if let Some(name) = repo_name(&window_text(state.url)) {
                        let proposed = wide(&state.parent.join(name).display().to_string());
                        state.filling = true;
                        SetWindowTextW(state.dir, proposed.as_ptr());
                        state.filling = false;
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
                Some(0)
            }
            WM_LBUTTONDOWN => {
                let (x, y) = ((lparam & 0xFFFF) as i16 as i32, ((lparam >> 16) & 0xFFFF) as i16 as i32);
                match state.buttons.iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, button)| *button) {
                    Some(Button::Browse) => state.browse = true,
                    Some(Button::Cancel) => state.done = Some(false),
                    Some(Button::Clone) => state.done = Some(true),
                    None => {}
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

/// Mostra il dialogo; URL e cartella di destinazione se l'utente clona.
pub fn ask(owner: HWND, palette: &Palette, font: HFONT, parent: PathBuf) -> Option<(String, PathBuf)> {
    let class_name = wide(CLASS_NAME);
    // SAFETY: finestra modale creata, usata e distrutta qui; il proprietario è disabilitato per tutta la durata.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSEXW { cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(clone_proc), hInstance: instance, hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW), lpszClassName: class_name.as_ptr(), ..std::mem::zeroed() };
        RegisterClassExW(&class);
        let s = |value: i32| value * GetDpiForWindow(owner) as i32 / 96;
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(owner, &mut client);
        let mut origin = POINT { x: 0, y: 0 };
        ClientToScreen(owner, &mut origin);
        let width = s(WIDTH).min(client.right - s(32)).max(s(360));
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), std::ptr::null(), WS_POPUP, origin.x + (client.right - width) / 2, origin.y + client.bottom * 12 / 100, width, s(HEIGHT), owner, std::ptr::null_mut(), instance, std::ptr::null());
        SendMessageW(hwnd, WM_SETFONT, font as usize, 0);
        let edit_class = wide("EDIT");
        let face = wide("Cascadia Mono");
        let mono = CreateFontW(-s(12), 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, face.as_ptr());
        let url = CreateWindowExW(0, edit_class.as_ptr(), std::ptr::null(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32, s(21), s(66), width - s(42), s(20), hwnd, std::ptr::null_mut(), instance, std::ptr::null());
        let browse_width = s(84);
        let dir = CreateWindowExW(0, edit_class.as_ptr(), std::ptr::null(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32, s(21), s(122), width - s(42) - browse_width - s(8), s(20), hwnd, std::ptr::null_mut(), instance, std::ptr::null());
        SendMessageW(url, WM_SETFONT, mono as usize, 0);
        SendMessageW(dir, WM_SETFONT, font as usize, 0);
        let cue = wide("git@github.com:utente/progetto.git oppure https://…");
        SendMessageW(url, 0x1501, 1, cue.as_ptr() as isize);
        STATE.with(|state| *state.borrow_mut() = Some(State { palette: *palette, brush: CreateSolidBrush(palette.editor), url, dir, parent, touched: false, filling: false, buttons: Vec::new(), done: None, browse: false }));

        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(url);
        let mut msg: MSG = std::mem::zeroed();
        let accepted = loop {
            if STATE.with(|state| state.borrow_mut().as_mut().is_some_and(|state| std::mem::take(&mut state.browse))) {
                if let Some(folder) = pick_folder(hwnd) {
                    let name = repo_name(&window_text(url));
                    let target = name.map_or_else(|| folder.clone(), |name| folder.join(name));
                    let text = wide(&target.display().to_string());
                    SetWindowTextW(dir, text.as_ptr());
                    STATE.with(|state| {
                        if let Some(state) = state.borrow_mut().as_mut() {
                            state.touched = true;
                            state.parent = folder;
                        }
                    });
                }
            }
            if let Some(done) = STATE.with(|state| state.borrow().as_ref().and_then(|state| state.done)) {
                break done;
            }
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                // WM_QUIT arrivato durante il dialogo: va rimesso in coda per il ciclo principale.
                PostQuitMessage(0);
                break false;
            }
            if msg.message == WM_KEYDOWN {
                match msg.wParam as u16 {
                    VK_RETURN if valid(&window_text(url), &window_text(dir)) => break true,
                    VK_RETURN => continue,
                    VK_ESCAPE => break false,
                    VK_TAB => {
                        SetFocus(if GetFocus() == url { dir } else { url });
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                        continue;
                    }
                    _ => {}
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        };
        let (url_text, dir_text) = (window_text(url).trim().to_string(), window_text(dir).trim().to_string());
        let state = STATE.with(|state| state.borrow_mut().take());
        EnableWindow(owner, 1);
        DestroyWindow(hwnd);
        DeleteObject(mono);
        if let Some(state) = state {
            DeleteObject(state.brush);
        }
        SetFocus(owner);
        (accepted && valid(&url_text, &dir_text)).then(|| (url_text, PathBuf::from(dir_text)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_the_target_folder() {
        let dir = std::env::temp_dir().join(format!("steroitor-clone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(target_state(&dir), Target::Empty);
        assert_eq!(target_state(&dir.join("nuova")), Target::Missing);
        std::fs::write(dir.join("file"), "").unwrap();
        assert_eq!(target_state(&dir), Target::NotEmpty);
        assert_eq!(target_state(&dir.join("file")), Target::File);
        assert!(valid("https://x/y.git", &dir.join("nuova").to_string_lossy()) && !valid("https://x/y.git", &dir.to_string_lossy()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

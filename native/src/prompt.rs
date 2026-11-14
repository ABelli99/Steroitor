//! Finestra modale con un campo di testo (es. il nome di un nuovo branch): Invio conferma, Esc annulla.
use std::cell::Cell;

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_RETURN};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::find::window_text;
use crate::paint::{fill, text};
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorPrompt";
const WIDTH: i32 = 420;
const HEIGHT: i32 = 130;
const MULTILINE_HEIGHT: i32 = 300;

thread_local! {
    static STATE: Cell<Option<(Palette, *const u16, HBRUSH, bool)>> = const { Cell::new(None) };
}

unsafe extern "system" fn prompt_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_PAINT => {
            let Some((palette, label, _, multiline)) = STATE.with(Cell::get) else { return DefWindowProcW(hwnd, message, wparam, lparam) };
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(hwnd, &mut client);
            fill(dc, &client, palette.panel);
            let scale = GetDpiForWindow(hwnd) as i32;
            let font = SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT;
            let previous = SelectObject(dc, font);
            SetBkMode(dc, TRANSPARENT as i32);
            let length = (0..).take_while(|&index| *label.add(index) != 0).count();
            let label = String::from_utf16_lossy(std::slice::from_raw_parts(label, length));
            let area = RECT { left: 16 * scale / 96, top: 12 * scale / 96, right: client.right - 16 * scale / 96, bottom: 36 * scale / 96 };
            text(dc, &label, &area, palette.fg, DT_LEFT);
            let hint = RECT { top: client.bottom - 32 * scale / 96, bottom: client.bottom - 8 * scale / 96, ..area };
            let hint_text = if multiline { "Ctrl+Invio per confermare, Esc per annullare" } else { "Invio per confermare, Esc per annullare" };
            text(dc, hint_text, &hint, palette.muted, DT_LEFT);
            SelectObject(dc, previous);
            EndPaint(hwnd, &paint);
            0
        }
        WM_CTLCOLOREDIT => {
            let Some((palette, _, brush, _)) = STATE.with(Cell::get) else { return 0 };
            SetTextColor(wparam as HDC, palette.fg);
            SetBkColor(wparam as HDC, palette.editor);
            brush as LRESULT
        }
        WM_CLOSE => {
            PostMessageW(hwnd, WM_APP, 0, 0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

/// Chiede un testo all'utente; None se annulla o lascia il campo vuoto.
pub fn prompt(owner: HWND, palette: &Palette, font: HFONT, title: &str, label: &str, initial: &str) -> Option<String> {
    ask(owner, palette, font, title, label, initial, false, None)
}

/// Come `prompt`, con selezionati solo i primi `selected` caratteri (UTF-16): per un nome, fino all'estensione.
pub fn prompt_selected(owner: HWND, palette: &Palette, font: HFONT, title: &str, label: &str, initial: &str, selected: usize) -> Option<String> {
    ask(owner, palette, font, title, label, initial, false, Some(selected))
}

/// Come `prompt`, con un campo su più righe (es. un messaggio di commit): Invio va a capo, Ctrl+Invio conferma.
pub fn prompt_text(owner: HWND, palette: &Palette, font: HFONT, title: &str, label: &str, initial: &str) -> Option<String> {
    let initial = initial.replace("\r\n", "\n").replace('\n', "\r\n");
    ask(owner, palette, font, title, label, &initial, true, None).map(|text| text.replace("\r\n", "\n"))
}

fn ask(owner: HWND, palette: &Palette, font: HFONT, title: &str, label: &str, initial: &str, multiline: bool, selection: Option<usize>) -> Option<String> {
    let class_name = wide(CLASS_NAME);
    let (title, label, initial) = (wide(title), wide(label), wide(initial));
    // SAFETY: finestra modale creata, usata e distrutta qui; il proprietario è disabilitato per tutta la durata.
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(prompt_proc),
            hInstance: instance,
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&class);
        let brush = CreateSolidBrush(palette.editor);
        STATE.with(|state| state.set(Some((*palette, label.as_ptr(), brush, multiline))));

        let scale = GetDpiForWindow(owner) as i32;
        let (width, height) = (WIDTH * scale / 96, if multiline { MULTILINE_HEIGHT } else { HEIGHT } * scale / 96);
        let mut owner_rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(owner, &mut owner_rect);
        let (left, top) = (owner_rect.left + (owner_rect.right - owner_rect.left - width) / 2, owner_rect.top + (owner_rect.bottom - owner_rect.top - height) / 3);
        let hwnd = CreateWindowExW(WS_EX_DLGMODALFRAME, class_name.as_ptr(), title.as_ptr(), WS_POPUP | WS_CAPTION | WS_SYSMENU, left, top, width, height, owner, std::ptr::null_mut(), instance, std::ptr::null());
        let dark: i32 = palette.dark.into();
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, (&dark as *const i32).cast(), 4);
        SendMessageW(hwnd, WM_SETFONT, font as usize, 0);
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(hwnd, &mut client);
        let edit_class = wide("EDIT");
        let style = if multiline { WS_VSCROLL | (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 } else { ES_AUTOHSCROLL as u32 };
        let edit_height = if multiline { client.bottom - 80 * scale / 96 } else { 26 * scale / 96 };
        let edit = CreateWindowExW(0, edit_class.as_ptr(), initial.as_ptr(), WS_CHILD | WS_VISIBLE | WS_BORDER | style, 16 * scale / 96, 40 * scale / 96, client.right - 32 * scale / 96, edit_height, hwnd, std::ptr::null_mut(), instance, std::ptr::null());
        SendMessageW(edit, WM_SETFONT, font as usize, 0);
        SendMessageW(edit, 0x00B1, 0, selection.map_or(-1, |end| end as isize));

        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(edit);
        let mut accepted = false;
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                // WM_QUIT arrivato durante il dialogo: va rimesso in coda per il ciclo principale.
                PostQuitMessage(0);
                break;
            }
            if msg.hwnd == hwnd && msg.message == WM_APP {
                break;
            }
            let key = msg.wParam as u16;
            let confirms = key == VK_RETURN && (!multiline || GetKeyState(VK_CONTROL as i32) < 0);
            if msg.message == WM_KEYDOWN && (confirms || key == VK_ESCAPE) {
                accepted = confirms;
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let value = window_text(edit).trim().to_string();
        EnableWindow(owner, 1);
        DestroyWindow(hwnd);
        DeleteObject(brush);
        STATE.with(|state| state.set(None));
        SetFocus(owner);
        (accepted && !value.is_empty()).then_some(value)
    }
}

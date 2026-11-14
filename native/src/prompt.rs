//! Dialogo con un campo di testo, come il PromptDialog della versione Tauri: riquadro senza barra del titolo al 15%
//! dall'alto della finestra, titolo, campo con il bordo accent. Invio conferma, Esc annulla; nella variante su più
//! righe Invio va a capo e Ctrl+Invio conferma.
use std::cell::RefCell;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_RETURN};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::find::window_text;
use crate::paint::{fill, frame, text};
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorPrompt";
const WIDTH: i32 = 560;
const PADDING: i32 = 12;
const LINE: i32 = 22;
const FIELD: i32 = 28;
const MULTILINE_FIELD: i32 = 120;

struct State {
    palette: Palette,
    title: String,
    label: String,
    multiline: bool,
    brush: HBRUSH,
    edit: HWND,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

unsafe extern "system" fn prompt_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let handled = STATE.with(|state| {
        let state = state.try_borrow().ok()?;
        let state = state.as_ref()?;
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
                let mut top = s(PADDING);
                text(dc, &state.title, &RECT { left: s(PADDING), top, right: client.right - s(PADDING), bottom: top + s(LINE) }, palette.fg, DT_LEFT | DT_END_ELLIPSIS);
                top += s(LINE);
                if !state.label.is_empty() {
                    text(dc, &state.label, &RECT { left: s(PADDING), top, right: client.right - s(PADDING), bottom: top + s(LINE) }, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
                }
                // Bordo accent attorno al campo.
                let mut field = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                GetWindowRect(state.edit, &mut field);
                let mut origin = POINT { x: field.left, y: field.top };
                ScreenToClient(hwnd, &mut origin);
                let border = RECT { left: origin.x - s(5), top: origin.y - s(4), right: origin.x + field.right - field.left + s(5), bottom: origin.y + field.bottom - field.top + s(4) };
                fill(dc, &border, palette.editor);
                frame(dc, &border, palette.accent);
                if state.multiline {
                    text(dc, "Ctrl+Invio per confermare, Esc per annullare", &RECT { left: s(PADDING), top: client.bottom - s(PADDING + LINE), right: client.right - s(PADDING), bottom: client.bottom - s(PADDING) }, palette.muted, DT_LEFT);
                }
                SelectObject(dc, previous);
                EndPaint(hwnd, &paint);
                Some(0)
            }
            WM_CTLCOLOREDIT => {
                SetTextColor(wparam as HDC, state.palette.fg);
                SetBkColor(wparam as HDC, state.palette.editor);
                Some(state.brush as LRESULT)
            }
            WM_CLOSE => {
                PostMessageW(hwnd, WM_APP, 0, 0);
                Some(0)
            }
            _ => None,
        }
    });
    handled.unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
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

#[allow(clippy::too_many_arguments)]
fn ask(owner: HWND, palette: &Palette, font: HFONT, title: &str, label: &str, initial: &str, multiline: bool, selection: Option<usize>) -> Option<String> {
    let class_name = wide(CLASS_NAME);
    let initial = wide(initial);
    // SAFETY: finestra modale creata, usata e distrutta qui; il proprietario è disabilitato per tutta la durata.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSEXW { cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(prompt_proc), hInstance: instance, hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW), lpszClassName: class_name.as_ptr(), ..std::mem::zeroed() };
        RegisterClassExW(&class);
        let s = |value: i32| value * GetDpiForWindow(owner) as i32 / 96;
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(owner, &mut client);
        let mut origin = POINT { x: 0, y: 0 };
        ClientToScreen(owner, &mut origin);
        let width = s(WIDTH).min(client.right - s(32)).max(s(240));
        let label_line = if label.is_empty() { 0 } else { s(LINE) };
        let field = s(if multiline { MULTILINE_FIELD } else { FIELD });
        let height = s(PADDING) + s(LINE) + label_line + s(8) + field + s(8) + if multiline { s(LINE + 4) } else { 0 } + s(PADDING);
        let left = origin.x + (client.right - width) / 2;
        let top = origin.y + client.bottom * 15 / 100;
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), std::ptr::null(), WS_POPUP, left, top, width, height, owner, std::ptr::null_mut(), instance, std::ptr::null());
        SendMessageW(hwnd, WM_SETFONT, font as usize, 0);
        let edit_class = wide("EDIT");
        let style = if multiline { WS_VSCROLL | (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 } else { ES_AUTOHSCROLL as u32 };
        let field_top = s(PADDING) + s(LINE) + label_line + s(8);
        let edit = CreateWindowExW(0, edit_class.as_ptr(), initial.as_ptr(), WS_CHILD | WS_VISIBLE | style, s(PADDING) + s(6), field_top + s(4), width - 2 * s(PADDING) - s(12), field - s(8), hwnd, std::ptr::null_mut(), instance, std::ptr::null());
        let mono = if multiline {
            let face = wide("Cascadia Mono");
            CreateFontW(-s(12), 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, face.as_ptr())
        } else {
            std::ptr::null_mut()
        };
        SendMessageW(edit, WM_SETFONT, if multiline { mono as usize } else { font as usize }, 0);
        SendMessageW(edit, 0x00B1, 0, selection.map_or(-1, |end| end as isize));
        let brush = CreateSolidBrush(palette.editor);
        STATE.with(|state| *state.borrow_mut() = Some(State { palette: *palette, title: title.to_string(), label: label.to_string(), multiline, brush, edit }));

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
        if !mono.is_null() {
            DeleteObject(mono);
        }
        STATE.with(|state| state.borrow_mut().take());
        SetFocus(owner);
        (accepted && !value.is_empty()).then_some(value)
    }
}

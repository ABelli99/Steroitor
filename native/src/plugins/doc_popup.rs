//! Riquadro della documentazione accanto alla lista dei suggerimenti. Non è un calltip di Scintilla perché
//! mostrare un calltip chiude la lista.
use std::cell::RefCell;

use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::paint::{fill, frame};
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorDocPopup";
const MAX_WIDTH: i32 = 440;
const PADDING: i32 = 8;

struct State {
    text: Vec<u16>,
    font: HFONT,
    palette: Palette,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

unsafe extern "system" fn popup_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(hwnd, &mut client);
            paint_state(hwnd, dc, &client);
            EndPaint(hwnd, &paint);
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn paint_state(hwnd: HWND, dc: HDC, client: &RECT) {
    STATE.with(|state| {
        let state = state.borrow();
        let Some(state) = state.as_ref() else { return };
        let padding = PADDING * GetDpiForWindow(hwnd) as i32 / 96;
        fill(dc, client, state.palette.panel);
        frame(dc, client, state.palette.border);
        let previous = SelectObject(dc, state.font);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, state.palette.fg);
        let mut text = state.text.clone();
        let mut area = RECT { left: padding, top: padding, right: client.right - padding, bottom: client.bottom - padding };
        DrawTextW(dc, text.as_mut_ptr(), text.len() as i32, &mut area, DT_WORDBREAK | DT_NOPREFIX | DT_EXPANDTABS);
        SelectObject(dc, previous);
    });
}

unsafe extern "system" fn find_list(hwnd: HWND, found: LPARAM) -> BOOL {
    let mut class = [0u16; 16];
    let length = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) as usize;
    if IsWindowVisible(hwnd) != 0 && String::from_utf16_lossy(&class[..length]) == "ListBoxX" {
        *(found as *mut HWND) = hwnd;
        return 0;
    }
    1
}

/// La lista dei suggerimenti di Scintilla aperta su questo thread.
fn list_rect() -> Option<RECT> {
    let mut list: HWND = std::ptr::null_mut();
    // SAFETY: enumerazione delle finestre del thread; il callback scrive solo in `list`.
    unsafe {
        EnumThreadWindows(GetCurrentThreadId(), Some(find_list), &mut list as *mut HWND as LPARAM);
        if list.is_null() {
            return None;
        }
        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(list, &mut rect);
        Some(rect)
    }
}

pub struct DocPopup {
    owner: HWND,
    hwnd: RefCell<HWND>,
}

impl DocPopup {
    pub fn new(owner: HWND) -> DocPopup {
        DocPopup { owner, hwnd: RefCell::new(std::ptr::null_mut()) }
    }

    fn window(&self) -> HWND {
        let mut hwnd = self.hwnd.borrow_mut();
        if !hwnd.is_null() {
            return *hwnd;
        }
        let class_name = wide(CLASS_NAME);
        // SAFETY: classe e finestra create una volta, sul thread della UI.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSEXW { cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(popup_proc), hInstance: instance, hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW), lpszClassName: class_name.as_ptr(), ..std::mem::zeroed() };
            RegisterClassExW(&class);
            *hwnd = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, class_name.as_ptr(), std::ptr::null(), WS_POPUP, 0, 0, 0, 0, self.owner, std::ptr::null_mut(), instance, std::ptr::null());
        }
        *hwnd
    }

    /// Mostra il testo a destra della lista (a sinistra se non c'è spazio); nulla se la lista non è aperta.
    pub fn show(&self, text: &str, font: HFONT, palette: &Palette) {
        let Some(list) = list_rect() else { return self.hide() };
        let hwnd = self.window();
        let mut wide_text: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: finestra valida; il DC viene rilasciato subito dopo la misura del testo.
        unsafe {
            let scale = |value: i32| value * GetDpiForWindow(self.owner) as i32 / 96;
            let dc = GetDC(hwnd);
            let previous = SelectObject(dc, font);
            let mut measure = RECT { left: 0, top: 0, right: scale(MAX_WIDTH), bottom: 0 };
            DrawTextW(dc, wide_text.as_mut_ptr(), wide_text.len() as i32, &mut measure, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX | DT_EXPANDTABS);
            SelectObject(dc, previous);
            ReleaseDC(hwnd, dc);
            let (width, height) = (measure.right + scale(PADDING) * 2, (measure.bottom + scale(PADDING) * 2).min(scale(400)));
            let screen = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let left = if list.right + width <= screen { list.right } else { list.left - width };
            STATE.with(|state| *state.borrow_mut() = Some(State { text: wide_text, font, palette: *palette }));
            SetWindowPos(hwnd, HWND_TOP, left, list.top, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    }

    pub fn hide(&self) {
        let hwnd = *self.hwnd.borrow();
        if !hwnd.is_null() {
            // SAFETY: finestra valida creata da questo thread.
            unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
    }
}

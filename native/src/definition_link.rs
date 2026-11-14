//! Ctrl+mouse nell'editor, come in IntelliJ e nella versione Tauri: il simbolo sotto il mouse diventa un link
//! sottolineato e il clic porta alla dichiarazione. Il subclass gira gli eventi alla finestra principale, che sa
//! cosa è navigabile; senza un simbolo il Ctrl+clic resta quello di Scintilla (un cursore in più).
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const WM_EDITOR_LINK: u32 = WM_APP + 42;
/// wparam di WM_EDITOR_LINK; lparam è il punto nel client dell'editor, come nei messaggi del mouse.
pub const HOVER: usize = 0;
pub const CLICK: usize = 1;
pub const CLEAR: usize = 2;
/// Risponde 1 se c'è un link attivo (cursore a mano).
pub const ACTIVE: usize = 3;

const MK_CONTROL: usize = 0x0008;
const MK_SHIFT: usize = 0x0004;

unsafe extern "system" fn editor_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _: usize, main: usize) -> LRESULT {
    let main = main as HWND;
    match message {
        WM_MOUSEMOVE => {
            SendMessageW(main, WM_EDITOR_LINK, if wparam & MK_CONTROL != 0 { HOVER } else { CLEAR }, lparam);
        }
        WM_LBUTTONDOWN if wparam & MK_CONTROL != 0 && wparam & MK_SHIFT == 0 && GetKeyState(VK_MENU as i32) >= 0 => {
            if SendMessageW(main, WM_EDITOR_LINK, CLICK, lparam) != 0 {
                return 0;
            }
        }
        WM_KEYUP if wparam == VK_CONTROL as usize => {
            SendMessageW(main, WM_EDITOR_LINK, CLEAR, 0);
        }
        WM_MOUSELEAVE | WM_KILLFOCUS => {
            SendMessageW(main, WM_EDITOR_LINK, CLEAR, 0);
        }
        WM_SETCURSOR if SendMessageW(main, WM_EDITOR_LINK, ACTIVE, 0) != 0 => {
            SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_HAND));
            return 1;
        }
        _ => {}
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

pub fn attach(editor: HWND, main: HWND) {
    // SAFETY: il subclass vive quanto l'editor, figlio della finestra principale.
    unsafe { SetWindowSubclass(editor, Some(editor_proc), 2, main as usize) };
}

//! Minimap: una seconda vista Scintilla sullo stesso documento dell'editor, a caratteri minuscoli, con i marker Git
//! nel margine. Non si modifica: clic e trascinamento spostano l'editor, la rotella va all'editor.
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::lang::{self, Language};
use crate::sci::*;
use crate::theme::Palette;

/// Riga del documento scelta nella minimap (wParam), da portare al centro dell'editor.
pub const WM_MINIMAP_LINE: u32 = WM_APP + 40;
pub const WIDTH: i32 = 110;
/// Caratteri a 1,5 punti (centesimi di punto per SCI_STYLESETSIZEFRACTIONAL).
const FONT_FRACTION: isize = 150;
const SCI_POSITIONFROMPOINT: u32 = 2022;
const SCI_GETLINECOUNT: u32 = 2154;
const SCI_LINESONSCREEN: u32 = 2370;
const SCI_DOCLINEFROMVISIBLE: u32 = 2221;
const SCI_SETVSCROLLBAR: u32 = 2280;
const SCI_SETHSCROLLBAR: u32 = 2130;
const SCI_SETCARETSTYLE: u32 = 2512;

pub struct Minimap {
    pub sci: Sci,
}

unsafe extern "system" fn minimap_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _: usize, editor: usize) -> LRESULT {
    const MK_LBUTTON: usize = 0x0001;
    let dragging = message == WM_MOUSEMOVE && wparam & MK_LBUTTON != 0;
    match message {
        WM_LBUTTONDOWN | WM_MOUSEMOVE if message == WM_LBUTTONDOWN || dragging => {
            if message == WM_LBUTTONDOWN {
                SetCapture(hwnd);
            }
            let y = ((lparam >> 16) & 0xFFFF) as i16 as isize;
            let position = SendMessageW(hwnd, SCI_POSITIONFROMPOINT, 0, (y.max(0) << 16) as LPARAM);
            let line = SendMessageW(hwnd, SCI_LINEFROMPOSITION, position.max(0) as usize, 0);
            PostMessageW(GetParent(hwnd), WM_MINIMAP_LINE, line.max(0) as usize, 0);
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            0
        }
        WM_MOUSEWHEEL => SendMessageW(editor as HWND, message, wparam, lparam),
        WM_SETFOCUS => {
            SetFocus(editor as HWND);
            0
        }
        WM_SETCURSOR => {
            SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_ARROW));
            1
        }
        WM_MOUSEMOVE | WM_KEYDOWN | WM_KEYUP | WM_CHAR | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_LBUTTONDBLCLK | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_CONTEXTMENU => 0,
        _ => DefSubclassProc(hwnd, message, wparam, lparam),
    }
}

impl Minimap {
    pub fn create(parent: HWND, instance: HINSTANCE, editor: HWND, palette: &Palette) -> Minimap {
        let class = wide("Scintilla");
        // SAFETY: controllo figlio della finestra principale; il subclass vive quanto il controllo.
        let hwnd = unsafe {
            let hwnd = CreateWindowExW(0, class.as_ptr(), std::ptr::null(), WS_CHILD, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null());
            if palette.dark {
                let theme = wide("DarkMode_Explorer");
                SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null());
            }
            SetWindowSubclass(hwnd, Some(minimap_proc), 1, editor as usize);
            hwnd
        };
        let sci = Sci::new(hwnd);
        sci.call(SCI_SETVSCROLLBAR, 0, 0);
        sci.call(SCI_SETHSCROLLBAR, 0, 0);
        sci.call(SCI_SETCARETSTYLE, 0, 0);
        sci.call(SCI_SETCARETLINEVISIBLE, 0, 0);
        sci.call(SCI_SETMARGINTYPEN, 1, SC_MARGIN_SYMBOL);
        sci.call(SCI_SETMARGINMASKN, 1, 0b111);
        sci.call(SCI_SETMARGINWIDTHN, 1, 3);
        sci.call(SCI_SETMARGINBACKN, 1, palette.editor as isize);
        let shade = (palette.active_line | 0xFF00_0000) as isize;
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_BACK, shade);
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_INACTIVE_BACK, shade);
        Minimap { sci }
    }

    /// Stessi marker Git dell'editor: i marker stanno nel documento, la minimap li mostra nel suo margine.
    pub fn define_marker(&self, marker: usize, symbol: isize, color: u32) {
        self.sci.call(SCI_MARKERDEFINE, marker, symbol);
        self.sci.call(SCI_MARKERSETBACK, marker, color as isize);
    }

    /// Mostra il documento dell'editor con gli stili del suo linguaggio (gli stili sono della vista).
    pub fn show_document(&self, pointer: isize, language: &Language, palette: &Palette, wrap: bool) {
        if self.sci.call(SCI_GETDOCPOINTER, 0, 0) != pointer {
            self.sci.call(SCI_SETDOCPOINTER, 0, pointer);
        }
        lang::apply_styles(&self.sci, language, palette, FONT_FRACTION);
        self.sci.call(SCI_SETMARGINWIDTHN, 0, 0);
        self.sci.call(SCI_SETWRAPMODE, if wrap { SC_WRAP_WORD } else { 0 }, 0);
    }

    /// Scorre la minimap in proporzione all'editor e segna le righe che l'editor mostra.
    pub fn sync(&self, editor: &Sci) {
        let first = editor.call(SCI_DOCLINEFROMVISIBLE, editor.call(SCI_GETFIRSTVISIBLELINE, 0, 0) as usize, 0).max(0);
        let shown = editor.call(SCI_LINESONSCREEN, 0, 0).max(1);
        let total = editor.call(SCI_GETLINECOUNT, 0, 0).max(1);
        let last = (first + shown).min(total);
        let line_start = |line: isize| self.sci.call(SCI_POSITIONFROMLINE, line as usize, 0);
        let end = if last >= total { self.sci.call(SCI_GETLENGTH, 0, 0) } else { line_start(last) };
        self.sci.call(SCI_SETSEL, line_start(first) as usize, end);
        let room = self.sci.call(SCI_LINESONSCREEN, 0, 0).max(1);
        let top = if total <= room { 0 } else { first * (total - room) / (total - shown).max(1) };
        self.sci.call(SCI_SETFIRSTVISIBLELINE, top.min(total - room).max(0) as usize, 0);
    }
}

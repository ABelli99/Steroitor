//! Lista virtuale disegnata dall'app (log dei commit, file da committare): scorrimento, selezione, tastiera.
//! Le righe le disegna il genitore, che riceve `WM_LIST_PAINT`; clic, doppio clic, menu e tasti arrivano come `WM_LIST_EVENT`.
use std::cell::Cell;

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::{SetScrollInfo, SetWindowTheme};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_DOWN, VK_END, VK_HOME, VK_NEXT, VK_PRIOR, VK_RETURN, VK_SPACE, VK_UP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;

pub const WM_LIST_PAINT: u32 = WM_APP + 10;
pub const WM_LIST_EVENT: u32 = WM_APP + 11;
const CLASS_NAME: &str = "SteroitorList";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ListEvent {
    Select,
    Activate,
    Toggle,
    Menu,
}

/// Richiesta di disegno di una riga, passata nel lParam di `WM_LIST_PAINT`.
pub struct RowPaint {
    pub list: HWND,
    pub dc: HDC,
    pub rect: RECT,
    pub index: usize,
    pub selected: bool,
    pub focused: bool,
}

pub struct List {
    pub hwnd: HWND,
    rows: Cell<usize>,
    top: Cell<usize>,
    selected: Cell<Option<usize>>,
    row_height: Cell<i32>,
    background: u32,
}

fn list_of(hwnd: HWND) -> Option<&'static List> {
    // SAFETY: GWLP_USERDATA contiene un &'static List impostato in `attach`, o 0.
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const List;
    // SAFETY: vedi sopra.
    unsafe { pointer.as_ref() }
}

impl List {
    pub fn create(parent: HWND, instance: HINSTANCE, background: u32, dark: bool) -> List {
        let class_name = wide(CLASS_NAME);
        // SAFETY: registrazione (ripetuta senza effetti) e creazione di un controllo figlio.
        let hwnd = unsafe {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(list_proc),
                hInstance: instance,
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                style: CS_DBLCLKS,
                ..std::mem::zeroed()
            };
            RegisterClassExW(&class);
            let hwnd = CreateWindowExW(0, class_name.as_ptr(), std::ptr::null(), WS_CHILD | WS_VSCROLL | WS_TABSTOP, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null());
            if dark {
                let theme = wide("DarkMode_Explorer");
                SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null());
            }
            hwnd
        };
        List { hwnd, rows: Cell::new(0), top: Cell::new(0), selected: Cell::new(None), row_height: Cell::new(22), background }
    }

    /// Da chiamare quando la lista ha il suo indirizzo definitivo (dentro l'app, che è 'static).
    pub fn attach(&'static self) {
        // SAFETY: il puntatore resta valido per tutta la vita del programma.
        unsafe { SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, self as *const List as isize) };
    }

    pub fn set_row_height(&self, height: i32) {
        self.row_height.set(height.max(1));
        self.update_scroll();
    }

    pub fn set_rows(&self, rows: usize) {
        self.rows.set(rows);
        if self.selected.get().is_some_and(|selected| selected >= rows) {
            self.selected.set(None);
        }
        self.top.set(self.top.get().min(rows.saturating_sub(1)));
        self.update_scroll();
        self.redraw();
    }

    pub fn redraw(&self) {
        // SAFETY: controllo valido.
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    fn visible_rows(&self) -> usize {
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: controllo valido.
        unsafe { GetClientRect(self.hwnd, &mut client) };
        (client.bottom / self.row_height.get()).max(1) as usize
    }

    fn update_scroll(&self) {
        // SAFETY: struttura compilata per intero.
        unsafe {
            let info = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                nMin: 0,
                nMax: self.rows.get().saturating_sub(1) as i32,
                nPage: self.visible_rows() as u32,
                nPos: self.top.get() as i32,
                nTrackPos: 0,
            };
            SetScrollInfo(self.hwnd, SB_VERT as i32, &info, 1);
        }
    }

    fn scroll_to(&self, top: isize) {
        let max = self.rows.get().saturating_sub(self.visible_rows()) as isize;
        self.top.set(top.clamp(0, max.max(0)) as usize);
        self.update_scroll();
        self.redraw();
    }

    fn select(&self, index: usize, event: ListEvent) {
        if self.rows.get() == 0 {
            return;
        }
        let index = index.min(self.rows.get() - 1);
        self.selected.set(Some(index));
        let visible = self.visible_rows();
        if index < self.top.get() {
            self.scroll_to(index as isize);
        } else if index >= self.top.get() + visible {
            self.scroll_to((index + 1 - visible) as isize);
        }
        self.redraw();
        self.notify(event, index);
    }

    fn notify(&self, event: ListEvent, index: usize) {
        // SAFETY: il genitore esiste finché esiste il controllo.
        unsafe {
            let parent = GetParent(self.hwnd);
            SendMessageW(parent, WM_LIST_EVENT, self.hwnd as usize, ((event as isize) << 32) | index as isize);
        }
    }

    fn row_at(&self, y: i32) -> Option<usize> {
        let index = self.top.get() + (y / self.row_height.get()).max(0) as usize;
        (index < self.rows.get()).then_some(index)
    }

    unsafe fn paint(&self) {
        let mut paint: PAINTSTRUCT = std::mem::zeroed();
        let target = BeginPaint(self.hwnd, &mut paint);
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(self.hwnd, &mut client);
        let dc = CreateCompatibleDC(target);
        let bitmap = CreateCompatibleBitmap(target, client.right.max(1), client.bottom.max(1));
        let previous = SelectObject(dc, bitmap);
        crate::paint::fill(dc, &client, self.background);
        let parent = GetParent(self.hwnd);
        let focused = windows_sys::Win32::UI::Input::KeyboardAndMouse::GetFocus() == self.hwnd;
        let height = self.row_height.get();
        let (top, rows) = (self.top.get(), self.rows.get());
        for (offset, index) in (top..rows).enumerate().take(self.visible_rows() + 1) {
            let rect = RECT { left: 0, top: offset as i32 * height, right: client.right, bottom: (offset as i32 + 1) * height };
            let request = RowPaint { list: self.hwnd, dc, rect, index, selected: self.selected.get() == Some(index), focused };
            SendMessageW(parent, WM_LIST_PAINT, 0, &request as *const RowPaint as isize);
        }
        BitBlt(target, 0, 0, client.right, client.bottom, dc, 0, 0, SRCCOPY);
        SelectObject(dc, previous);
        DeleteObject(bitmap);
        DeleteDC(dc);
        EndPaint(self.hwnd, &paint);
    }

    fn key(&self, key: u16) -> bool {
        let page = self.visible_rows();
        let current = self.selected.get();
        let target = match key {
            VK_UP => current.map_or(0, |index| index.saturating_sub(1)),
            VK_DOWN => current.map_or(0, |index| index + 1),
            VK_PRIOR => current.map_or(0, |index| index.saturating_sub(page)),
            VK_NEXT => current.map_or(0, |index| index + page),
            VK_HOME => 0,
            VK_END => self.rows.get().saturating_sub(1),
            VK_RETURN => {
                if let Some(index) = current {
                    self.notify(ListEvent::Activate, index);
                }
                return true;
            }
            VK_SPACE => {
                if let Some(index) = current {
                    self.notify(ListEvent::Toggle, index);
                }
                return true;
            }
            _ => return false,
        };
        self.select(target, ListEvent::Select);
        true
    }
}

unsafe extern "system" fn list_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(list) = list_of(hwnd) else { return DefWindowProcW(hwnd, message, wparam, lparam) };
    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
    match message {
        WM_PAINT => {
            list.paint();
            0
        }
        WM_ERASEBKGND => 1,
        WM_SIZE => {
            list.update_scroll();
            0
        }
        WM_VSCROLL => {
            let mut info: SCROLLINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
            info.fMask = SIF_TRACKPOS;
            GetScrollInfo(hwnd, SB_VERT as i32, &mut info);
            let top = list.top.get() as isize;
            let page = list.visible_rows() as isize;
            let target = match (wparam & 0xFFFF) as i32 {
                x if x == SB_LINEUP as i32 => top - 1,
                x if x == SB_LINEDOWN as i32 => top + 1,
                x if x == SB_PAGEUP as i32 => top - page,
                x if x == SB_PAGEDOWN as i32 => top + page,
                x if x == SB_THUMBTRACK as i32 || x == SB_THUMBPOSITION as i32 => info.nTrackPos as isize,
                _ => top,
            };
            list.scroll_to(target);
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xFFFF) as i16 as isize;
            list.scroll_to(list.top.get() as isize - delta / 40);
            0
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
            SetFocus(hwnd);
            if let Some(index) = list.row_at(y) {
                list.select(index, ListEvent::Select);
                if message == WM_RBUTTONDOWN {
                    list.notify(ListEvent::Menu, index);
                }
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            if let Some(index) = list.row_at(y) {
                list.notify(ListEvent::Activate, index);
            }
            0
        }
        WM_KEYDOWN => {
            if list.key(wparam as u16) {
                0
            } else {
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            list.redraw();
            0
        }
        WM_GETDLGCODE => DLGC_WANTARROWS as LRESULT,
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

/// Evento di una lista dal messaggio `WM_LIST_EVENT`.
pub fn decode_event(lparam: LPARAM) -> (ListEvent, usize) {
    let event = match lparam >> 32 {
        1 => ListEvent::Activate,
        2 => ListEvent::Toggle,
        3 => ListEvent::Menu,
        _ => ListEvent::Select,
    };
    (event, (lparam & 0xFFFF_FFFF) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_events_through_lparam() {
        for event in [ListEvent::Select, ListEvent::Activate, ListEvent::Toggle, ListEvent::Menu] {
            let lparam = ((event as isize) << 32) | 1234;
            assert_eq!(decode_event(lparam), (event, 1234));
        }
    }
}

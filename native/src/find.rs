//! Barra di ricerca e sostituzione sopra l'editor (`Ctrl+F`, `Ctrl+H`), con tutte le occorrenze evidenziate.
use std::cell::{Cell, RefCell};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::HDC;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::paint::{fill, frame, text, text_width};
use crate::sci::*;
use crate::theme::Palette;

/// Indicatore riservato ai container (INDIC_CONTAINER): non lo usa nessun lexer.
pub const INDICATOR: usize = 8;
const MAX_HIGHLIGHTS: usize = 10_000;
const ROW_HEIGHT: i32 = 34;
const EDIT_WIDTH: i32 = 320;
const BUTTON: i32 = 28;
const EM_SETSEL: u32 = 0x00B1;

#[derive(Clone, Copy, PartialEq)]
pub enum Button {
    Previous,
    Next,
    MatchCase,
    Close,
    ReplaceOne,
    ReplaceAll,
}

pub struct FindBar {
    pub find: HWND,
    pub replace: HWND,
    open: Cell<bool>,
    replacing: Cell<bool>,
    match_case: Cell<bool>,
    /// Occorrenza selezionata (1-based, 0 = nessuna) e totale.
    counts: Cell<(usize, usize)>,
    buttons: RefCell<Vec<(RECT, Button)>>,
}

fn edit(parent: HWND, instance: HINSTANCE) -> HWND {
    let class = wide("EDIT");
    // SAFETY: controllo figlio della finestra principale.
    unsafe { CreateWindowExW(0, class.as_ptr(), std::ptr::null(), WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL as u32, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null()) }
}

pub fn window_text(hwnd: HWND) -> String {
    // SAFETY: buffer della lunghezza dichiarata dal controllo, più il terminatore.
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..copied as usize])
    }
}

fn find_in(sci: &Sci, needle: &[u8], flags: usize, from: isize, to: isize) -> Option<(isize, isize)> {
    sci.call(SCI_SETSEARCHFLAGS, flags, 0);
    sci.call(SCI_SETTARGETRANGE, from as usize, to);
    let start = sci.call(SCI_SEARCHINTARGET, needle.len(), needle.as_ptr() as isize);
    (start >= 0).then(|| (start, sci.call(SCI_GETTARGETEND, 0, 0)))
}

impl FindBar {
    pub fn create(parent: HWND, instance: HINSTANCE) -> FindBar {
        FindBar {
            find: edit(parent, instance),
            replace: edit(parent, instance),
            open: Cell::new(false),
            replacing: Cell::new(false),
            match_case: Cell::new(false),
            counts: Cell::new((0, 0)),
            buttons: RefCell::new(Vec::new()),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    /// Altezza in pixel a 96 DPI.
    pub fn height(&self) -> i32 {
        match (self.open.get(), self.replacing.get()) {
            (false, _) => 0,
            (true, false) => ROW_HEIGHT,
            (true, true) => ROW_HEIGHT * 2,
        }
    }

    pub fn owns(&self, hwnd: HWND) -> bool {
        hwnd == self.find || hwnd == self.replace
    }

    pub fn show(&self, replacing: bool, initial: Option<String>) {
        self.open.set(true);
        self.replacing.set(replacing);
        // SAFETY: controlli validi; il testo vive per la chiamata.
        unsafe {
            if let Some(initial) = initial.filter(|text| !text.is_empty() && !text.contains('\n')) {
                let initial = wide(&initial);
                SetWindowTextW(self.find, initial.as_ptr());
            }
            ShowWindow(self.find, SW_SHOW);
            ShowWindow(self.replace, if replacing { SW_SHOW } else { SW_HIDE });
            SetFocus(self.find);
            SendMessageW(self.find, EM_SETSEL, 0, -1);
        }
    }

    pub fn hide(&self, sci: &Sci) {
        self.open.set(false);
        // SAFETY: controlli validi.
        unsafe {
            ShowWindow(self.find, SW_HIDE);
            ShowWindow(self.replace, SW_HIDE);
        }
        Self::clear(sci);
    }

    fn clear(sci: &Sci) {
        sci.call(SCI_SETINDICATORCURRENT, INDICATOR, 0);
        sci.call(SCI_INDICATORCLEARRANGE, 0, sci.call(SCI_GETLENGTH, 0, 0));
    }

    pub fn toggle_match_case(&self, sci: &Sci) {
        self.match_case.set(!self.match_case.get());
        self.highlight(sci);
    }

    fn needle(&self) -> Vec<u8> {
        window_text(self.find).into_bytes()
    }

    fn flags(&self) -> usize {
        if self.match_case.get() {
            SCFIND_MATCHCASE
        } else {
            0
        }
    }

    /// Seleziona l'occorrenza successiva (o precedente), ricominciando dall'inizio (o dalla fine).
    /// `incremental`: mentre si scrive si parte dall'inizio della selezione, così la corrispondenza resta quella.
    pub fn search(&self, sci: &Sci, forward: bool, incremental: bool) {
        let needle = self.needle();
        if needle.is_empty() {
            Self::clear(sci);
            self.counts.set((0, 0));
            return;
        }
        let length = sci.call(SCI_GETLENGTH, 0, 0);
        let (start, end) = (sci.call(SCI_GETSELECTIONSTART, 0, 0), sci.call(SCI_GETSELECTIONEND, 0, 0));
        let flags = self.flags();
        let found = if forward {
            let from = if incremental { start } else { end };
            find_in(sci, &needle, flags, from, length).or_else(|| find_in(sci, &needle, flags, 0, length))
        } else {
            find_in(sci, &needle, flags, start, 0).or_else(|| find_in(sci, &needle, flags, length, 0))
        };
        if let Some((from, to)) = found {
            sci.call(SCI_SETSEL, from as usize, to);
            sci.call(SCI_SCROLLCARET, 0, 0);
        }
        self.highlight(sci);
    }

    pub fn highlight(&self, sci: &Sci) {
        Self::clear(sci);
        let needle = self.needle();
        if needle.is_empty() {
            self.counts.set((0, 0));
            return;
        }
        let length = sci.call(SCI_GETLENGTH, 0, 0);
        let selection = sci.call(SCI_GETSELECTIONSTART, 0, 0);
        let (mut current, mut total, mut position) = (0, 0, 0);
        while let Some((from, to)) = find_in(sci, &needle, self.flags(), position, length) {
            if to <= from || total == MAX_HIGHLIGHTS {
                break;
            }
            sci.call(SCI_INDICATORFILLRANGE, from as usize, to - from);
            total += 1;
            if from == selection {
                current = total;
            }
            position = to;
        }
        self.counts.set((current, total));
    }

    /// Sostituisce l'occorrenza selezionata, poi passa alla successiva.
    pub fn replace_one(&self, sci: &Sci) {
        let needle = self.needle();
        let replacement = window_text(self.replace).into_bytes();
        let (start, end) = (sci.call(SCI_GETSELECTIONSTART, 0, 0), sci.call(SCI_GETSELECTIONEND, 0, 0));
        if !needle.is_empty() && find_in(sci, &needle, self.flags(), start, end) == Some((start, end)) {
            sci.call(SCI_TARGETFROMSELECTION, 0, 0);
            sci.call(SCI_REPLACETARGET, replacement.len(), replacement.as_ptr() as isize);
            sci.call(SCI_SETSEL, start as usize, start + replacement.len() as isize);
        }
        self.search(sci, true, false);
    }

    pub fn replace_all(&self, sci: &Sci) {
        let needle = self.needle();
        if needle.is_empty() {
            return;
        }
        let replacement = window_text(self.replace).into_bytes();
        sci.call(SCI_BEGINUNDOACTION, 0, 0);
        let mut position = 0;
        while let Some((from, to)) = find_in(sci, &needle, self.flags(), position, sci.call(SCI_GETLENGTH, 0, 0)) {
            if to <= from {
                break;
            }
            sci.call(SCI_REPLACETARGET, replacement.len(), replacement.as_ptr() as isize);
            position = sci.call(SCI_GETTARGETEND, 0, 0);
        }
        sci.call(SCI_ENDUNDOACTION, 0, 0);
        self.highlight(sci);
    }

    /// Posiziona i campi di testo nella barra (`rect` in pixel reali).
    pub fn layout(&self, rect: RECT, scale: impl Fn(i32) -> i32) {
        let row = scale(ROW_HEIGHT);
        let width = scale(EDIT_WIDTH).min(rect.right - rect.left - scale(240)).max(scale(80));
        let place = |hwnd: HWND, top: i32| {
            // SAFETY: controllo figlio valido.
            unsafe { MoveWindow(hwnd, rect.left + scale(10), top + scale(8), width, row - scale(16), 1) };
        };
        place(self.find, rect.top);
        place(self.replace, rect.top + row);
    }

    pub unsafe fn paint(&self, dc: HDC, rect: RECT, palette: &Palette, scale: impl Fn(i32) -> i32) {
        fill(dc, &rect, palette.panel);
        fill(dc, &RECT { top: rect.bottom - 1, ..rect }, palette.border);
        let row = scale(ROW_HEIGHT);
        let width = scale(EDIT_WIDTH).min(rect.right - rect.left - scale(240)).max(scale(80));
        let field = |top: i32| RECT { left: rect.left + scale(6), top: top + scale(5), right: rect.left + scale(14) + width, bottom: top + row - scale(5) };
        fill(dc, &field(rect.top), palette.editor);
        frame(dc, &field(rect.top), palette.border);
        if self.replacing.get() {
            fill(dc, &field(rect.top + row), palette.editor);
            frame(dc, &field(rect.top + row), palette.border);
        }

        let mut buttons = Vec::new();
        let mut left = field(rect.top).right + scale(8);
        let (current, total) = self.counts.get();
        let label = match (self.needle().is_empty(), total) {
            (true, _) => String::new(),
            (false, 0) => "Nessun risultato".into(),
            (false, _) => format!("{current}/{total}"),
        };
        let count_width = text_width(dc, &label).max(scale(70));
        let first = RECT { left, top: rect.top, right: left + count_width, bottom: rect.top + row };
        text(dc, &label, &first, if total == 0 && !label.is_empty() { palette.danger } else { palette.muted }, windows_sys::Win32::Graphics::Gdi::DT_LEFT);
        left += count_width + scale(4);
        let case_color = if self.match_case.get() { palette.accent } else { palette.muted };
        for (label, button, color) in [("↑", Button::Previous, palette.fg), ("↓", Button::Next, palette.fg), ("Aa", Button::MatchCase, case_color), ("×", Button::Close, palette.muted)] {
            let area = RECT { left, top: rect.top, right: left + scale(BUTTON), bottom: rect.top + row };
            text(dc, label, &area, color, windows_sys::Win32::Graphics::Gdi::DT_CENTER);
            buttons.push((area, button));
            left += scale(BUTTON);
        }
        if self.replacing.get() {
            let mut left = field(rect.top + row).right + scale(8);
            for (label, button) in [("Sostituisci", Button::ReplaceOne), ("Tutti", Button::ReplaceAll)] {
                let width = text_width(dc, label) + scale(16);
                let area = RECT { left, top: rect.top + row + scale(5), right: left + width, bottom: rect.top + 2 * row - scale(5) };
                frame(dc, &area, palette.border);
                text(dc, label, &area, palette.fg, windows_sys::Win32::Graphics::Gdi::DT_CENTER);
                buttons.push((area, button));
                left += width + scale(6);
            }
        }
        *self.buttons.borrow_mut() = buttons;
    }

    pub fn button_at(&self, x: i32, y: i32) -> Option<Button> {
        self.buttons.borrow().iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, button)| *button)
    }
}

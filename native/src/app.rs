//! Finestra principale: barra delle tab e status bar disegnate a mano, Scintilla al centro, un documento Scintilla per tab.
use std::cell::{Cell, OnceCell, RefCell};
use std::ffi::c_void;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_CONTROL, VK_F4, VK_SHIFT, VK_TAB};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use steroitor_core::text::{decode, encode};

use crate::lang::{self, Language};
use crate::sci::*;
use crate::theme::{system_palette, Palette};

extern "C" {
    fn steroitor_create_lexer(name: *const std::ffi::c_char) -> *mut c_void;
}

const CLASS_NAME: &str = "SteroitorMain";
const TAB_HEIGHT: i32 = 32;
const STATUS_HEIGHT: i32 = 24;
const TAB_PADDING: i32 = 12;
const TAB_MIN_WIDTH: i32 = 80;
const TAB_MAX_WIDTH: i32 = 220;
const CLOSE_SIZE: i32 = 16;
const UI_FONT_PX: i32 = 13;
const EDITOR_FONT_SIZE: usize = 1050;

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

struct Doc {
    pointer: isize,
    path: Option<PathBuf>,
    untitled: u32,
    encoding: String,
    bom: bool,
    eol: usize,
    language: &'static Language,
    dirty: bool,
    /// Selezione e scroll: in Scintilla appartengono alla vista, non al documento.
    anchor: isize,
    caret: isize,
    first_line: isize,
}

impl Doc {
    fn name(&self) -> String {
        match &self.path {
            Some(path) => path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned()),
            None => format!("Senza titolo {}", self.untitled),
        }
    }
}

pub struct App {
    hwnd: HWND,
    sci: Sci,
    palette: Palette,
    docs: RefCell<Vec<Doc>>,
    active: Cell<usize>,
    next_untitled: Cell<u32>,
    font: Cell<HFONT>,
    tabs: RefCell<Vec<(RECT, RECT)>>,
}

thread_local! {
    static APP: OnceCell<&'static App> = const { OnceCell::new() };
}

fn app() -> Option<&'static App> {
    APP.with(|cell| cell.get().copied())
}

fn eol_of(text: &str) -> usize {
    if text.contains("\r\n") {
        SC_EOL_CRLF
    } else if text.contains('\n') {
        SC_EOL_LF
    } else if text.contains('\r') {
        SC_EOL_CR
    } else {
        SC_EOL_CRLF
    }
}

fn eol_label(eol: usize) -> &'static str {
    match eol {
        SC_EOL_LF => "LF",
        SC_EOL_CR => "CR",
        _ => "CRLF",
    }
}

fn message_box(owner: HWND, text: &str, flags: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    let text = wide(text);
    let title = wide("Steroitor");
    // SAFETY: stringhe terminate da zero che vivono per tutta la chiamata.
    unsafe { MessageBoxW(owner, text.as_ptr(), title.as_ptr(), flags) }
}

impl App {
    pub fn create(instance: HINSTANCE, files: Vec<PathBuf>) -> &'static App {
        let palette = system_palette();
        let class_name = wide(CLASS_NAME);
        // SAFETY: chiamate Win32 con strutture inizializzate e stringhe terminate da zero.
        let (hwnd, editor) = unsafe {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                style: CS_HREDRAW | CS_VREDRAW,
                ..std::mem::zeroed()
            };
            RegisterClassExW(&class);
            let title = wide("Steroitor");
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1200,
                800,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            let dark: i32 = palette.dark.into();
            DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, (&dark as *const i32).cast(), 4);
            let scintilla = wide("Scintilla");
            let editor = CreateWindowExW(0, scintilla.as_ptr(), std::ptr::null(), WS_CHILD | WS_VISIBLE, 0, 0, 0, 0, hwnd, std::ptr::null_mut(), instance, std::ptr::null());
            if palette.dark {
                let theme = wide("DarkMode_Explorer");
                SetWindowTheme(editor, theme.as_ptr(), std::ptr::null());
            }
            (hwnd, editor)
        };

        let app: &'static App = Box::leak(Box::new(App {
            hwnd,
            sci: Sci::new(editor),
            palette,
            docs: RefCell::new(Vec::new()),
            active: Cell::new(0),
            next_untitled: Cell::new(1),
            font: Cell::new(std::ptr::null_mut()),
            tabs: RefCell::new(Vec::new()),
        }));
        APP.with(|cell| cell.set(app).ok());
        app.configure_editor();
        app.update_font();
        for file in files {
            app.open(&file);
        }
        if app.docs.borrow().is_empty() {
            app.new_document();
        }
        app.layout();
        // SAFETY: finestra appena creata da questo thread.
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
        }
        app.focus_editor();
        app
    }

    fn dpi(&self) -> i32 {
        // SAFETY: finestra valida.
        unsafe { GetDpiForWindow(self.hwnd) as i32 }
    }

    fn scale(&self, value: i32) -> i32 {
        value * self.dpi() / 96
    }

    fn update_font(&self) {
        let face = wide("Segoe UI");
        // SAFETY: il font precedente non è più selezionato in nessun DC quando lo si elimina.
        unsafe {
            let font = CreateFontW(-self.scale(UI_FONT_PX), 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, face.as_ptr());
            let old = self.font.replace(font);
            if !old.is_null() {
                DeleteObject(old);
            }
        }
    }

    fn configure_editor(&self) {
        let sci = &self.sci;
        sci.call(SCI_SETMULTIPLESELECTION, 1, 0);
        sci.call(SCI_SETADDITIONALSELECTIONTYPING, 1, 0);
        sci.call(SCI_SETMULTIPASTE, 1, 0);
        sci.call(SCI_SETSCROLLWIDTH, 1, 0);
        sci.call(SCI_SETSCROLLWIDTHTRACKING, 1, 0);
        sci.call(SCI_SETCARETLINEVISIBLE, 1, 0);
        sci.call(SCI_SETCARETLINEVISIBLEALWAYS, 1, 0);
        sci.call(SCI_SETCARETWIDTH, 2, 0);
        sci.call(SCI_SETINDENTATIONGUIDES, SC_IV_LOOKBOTH, 0);
        sci.call(SCI_SETMARGINTYPEN, 0, SC_MARGIN_NUMBER);
        sci.call(SCI_SETMARGINLEFT, 0, 4);
        sci.call(SCI_SETMODEVENTMASK, 0, 0);
        let opaque = |color: u32| (color | 0xFF00_0000) as isize;
        let palette = &self.palette;
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_CARET, opaque(palette.accent));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_CARET_LINE_BACK, opaque(palette.active_line));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_BACK, opaque(palette.selection));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_ADDITIONAL_BACK, opaque(palette.selection));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_INACTIVE_BACK, opaque(palette.selection));
        sci.call(SCI_SETMARGINBACKN, 0, palette.editor as isize);
    }

    /// Gli stili sono della vista: si riapplicano a ogni cambio di tab, perché ogni lexer numera gli stili a modo suo.
    fn apply_styles(&self, language: &Language) {
        let sci = &self.sci;
        let palette = &self.palette;
        sci.call_str(SCI_STYLESETFONT, STYLE_DEFAULT, "Cascadia Mono");
        sci.call(SCI_STYLESETSIZEFRACTIONAL, STYLE_DEFAULT, EDITOR_FONT_SIZE as isize);
        sci.call(SCI_STYLESETFORE, STYLE_DEFAULT, palette.fg as isize);
        sci.call(SCI_STYLESETBACK, STYLE_DEFAULT, palette.editor as isize);
        sci.call(SCI_STYLECLEARALL, 0, 0);
        sci.call(SCI_STYLESETFORE, STYLE_LINENUMBER, palette.muted as isize);
        sci.call(SCI_STYLESETBACK, STYLE_LINENUMBER, palette.editor as isize);
        sci.call(SCI_STYLESETFORE, STYLE_INDENTGUIDE, palette.border as isize);
        sci.call(SCI_STYLESETFORE, STYLE_BRACELIGHT, palette.accent as isize);
        sci.call(SCI_STYLESETBOLD, STYLE_BRACELIGHT, 1);
        sci.call(SCI_STYLESETFORE, STYLE_BRACEBAD, palette.danger as isize);
        for &(style, role) in language.styles {
            sci.style(style, role.color(palette));
            if role.italic() {
                sci.call(SCI_STYLESETITALIC, style, 1);
            }
        }
        let width = sci.call_str(SCI_TEXTWIDTH, STYLE_LINENUMBER, "_99999");
        sci.call(SCI_SETMARGINWIDTHN, 0, width);
    }

    fn set_language(&self, language: &'static Language) {
        let lexer = language.lexer.map_or(std::ptr::null_mut(), |name| {
            let name = std::ffi::CString::new(name).unwrap_or_default();
            // SAFETY: nome valido; il documento prende possesso del lexer restituito.
            unsafe { steroitor_create_lexer(name.as_ptr()) }
        });
        self.sci.call(SCI_SETILEXER, 0, lexer as isize);
        for (index, keywords) in language.keywords.iter().enumerate() {
            self.sci.call_str(SCI_SETKEYWORDS, index, keywords);
        }
        for (key, value) in language.properties {
            self.sci.set_property(key, value);
        }
    }

    /// Crea il documento Scintilla, lo rende attivo e lo aggiunge come nuova tab.
    fn add_document(&self, path: Option<PathBuf>, text: &str, encoding: String, bom: bool) {
        self.remember_view();
        let pointer = self.sci.call(SCI_CREATEDOCUMENT, 0, 0);
        self.sci.call(SCI_SETDOCPOINTER, 0, pointer);
        let language = path.as_deref().map_or(&lang::PLAIN, lang::detect);
        let eol = eol_of(text);
        let untitled = if path.is_none() { self.next_untitled.replace(self.next_untitled.get() + 1) } else { 0 };
        // In lista prima di caricare il testo: le notifiche di modifica devono arrivare a questa tab, non alla precedente.
        let index = {
            let mut docs = self.docs.borrow_mut();
            docs.push(Doc { pointer, path, untitled, encoding, bom, eol, language, dirty: false, anchor: 0, caret: 0, first_line: 0 });
            docs.len() - 1
        };
        self.active.set(index);
        self.sci.call(SCI_SETCODEPAGE, SC_CP_UTF8, 0);
        self.sci.call(SCI_SETEOLMODE, eol, 0);
        self.sci.call(SCI_SETTABWIDTH, 4, 0);
        self.sci.call(SCI_SETINDENT, 2, 0);
        self.sci.call(SCI_SETUSETABS, 0, 0);
        self.set_language(language);
        self.apply_styles(language);
        self.sci.set_text(text.as_bytes());
        self.refresh();
    }

    pub fn new_document(&self) {
        self.add_document(None, "", "UTF-8".into(), false);
    }

    pub fn open(&self, path: &Path) {
        let existing = self.docs.borrow().iter().position(|doc| doc.path.as_deref() == Some(path));
        if let Some(index) = existing {
            return self.activate(index);
        }
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                message_box(self.hwnd, &format!("Impossibile aprire {}:\n{error}", path.display()), MB_ICONERROR);
                return;
            }
        };
        let file = decode(&bytes);
        // Un'unica tab vuota e intatta (quella di avvio) lascia il posto al file aperto.
        let replace = {
            let docs = self.docs.borrow();
            docs.len() == 1 && docs[0].path.is_none() && !docs[0].dirty && self.sci.call(SCI_GETLENGTH, 0, 0) == 0
        };
        self.add_document(Some(path.to_path_buf()), &file.content, file.encoding, file.bom);
        if replace {
            self.remove(0);
        }
    }

    fn remember_view(&self) {
        let (anchor, caret, first_line) = (
            self.sci.call(SCI_GETANCHOR, 0, 0),
            self.sci.call(SCI_GETCURRENTPOS, 0, 0),
            self.sci.call(SCI_GETFIRSTVISIBLELINE, 0, 0),
        );
        if let Some(doc) = self.docs.borrow_mut().get_mut(self.active.get()) {
            (doc.anchor, doc.caret, doc.first_line) = (anchor, caret, first_line);
        }
    }

    pub fn activate(&self, index: usize) {
        if index == self.active.get() || index >= self.docs.borrow().len() {
            return;
        }
        self.remember_view();
        let (pointer, language, anchor, caret, first_line) = {
            let doc = &self.docs.borrow()[index];
            (doc.pointer, doc.language, doc.anchor, doc.caret, doc.first_line)
        };
        self.active.set(index);
        self.sci.call(SCI_SETDOCPOINTER, 0, pointer);
        self.apply_styles(language);
        self.sci.call(SCI_SETSEL, anchor as usize, caret);
        self.sci.call(SCI_SETFIRSTVISIBLELINE, first_line as usize, 0);
        self.refresh();
        self.focus_editor();
    }

    /// Toglie la tab senza chiedere nulla; se era l'ultima ne apre una vuota.
    fn remove(&self, index: usize) {
        if self.docs.borrow().len() == 1 {
            self.new_document();
        }
        let active = self.active.get();
        if index == active {
            let next = if index + 1 < self.docs.borrow().len() { index + 1 } else { index - 1 };
            self.activate(next);
        }
        let pointer = self.docs.borrow_mut().remove(index).pointer;
        self.sci.call(SCI_RELEASEDOCUMENT, 0, pointer);
        if self.active.get() > index {
            self.active.set(self.active.get() - 1);
        }
        self.refresh();
    }

    /// false se l'utente annulla, o se il salvataggio richiesto fallisce.
    fn confirm_discard(&self, index: usize) -> bool {
        let (dirty, name) = {
            let doc = &self.docs.borrow()[index];
            (doc.dirty, doc.name())
        };
        if !dirty {
            return true;
        }
        match message_box(self.hwnd, &format!("Salvare le modifiche a {name}?"), MB_YESNOCANCEL | MB_ICONWARNING) {
            IDYES => {
                self.activate(index);
                self.save(false)
            }
            IDNO => true,
            _ => false,
        }
    }

    pub fn close(&self, index: usize) {
        if self.confirm_discard(index) {
            self.remove(index);
        }
    }

    fn confirm_quit(&self) -> bool {
        (0..self.docs.borrow().len()).all(|index| self.confirm_discard(index))
    }

    /// Salva la tab attiva; con `ask` (o se non ha un percorso) chiede dove.
    pub fn save(&self, ask: bool) -> bool {
        let index = self.active.get();
        let (current, name, encoding, bom) = {
            let doc = &self.docs.borrow()[index];
            (doc.path.clone(), doc.name(), doc.encoding.clone(), doc.bom)
        };
        let path = match current.clone().filter(|_| !ask) {
            Some(path) => path,
            None => match save_dialog(self.hwnd, &name) {
                Some(path) => path,
                None => return false,
            },
        };
        let written = encode(&self.sci.text(), &encoding, bom).and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
        if let Err(error) = written {
            message_box(self.hwnd, &format!("Impossibile salvare {}:\n{error}", path.display()), MB_ICONERROR);
            return false;
        }
        if current.as_deref() != Some(path.as_path()) {
            let language = lang::detect(&path);
            {
                let mut docs = self.docs.borrow_mut();
                docs[index].path = Some(path);
                docs[index].language = language;
            }
            self.set_language(language);
            self.apply_styles(language);
        }
        self.sci.call(SCI_SETSAVEPOINT, 0, 0);
        self.refresh();
        true
    }

    fn cycle(&self, step: isize) {
        let count = self.docs.borrow().len() as isize;
        self.activate((self.active.get() as isize + step).rem_euclid(count) as usize);
    }

    fn focus_editor(&self) {
        self.sci.call(SCI_GRABFOCUS, 0, 0);
        // SAFETY: finestra figlia valida.
        unsafe {
            SetFocus(self.sci.hwnd);
        }
    }

    /// Scorciatoie dell'app, prima che il tasto arrivi a Scintilla. true se il tasto è stato usato.
    pub fn shortcut(&self, msg: &MSG) -> bool {
        if msg.message != WM_KEYDOWN {
            return false;
        }
        // SAFETY: lettura dello stato della tastiera del thread.
        let (ctrl, shift) = unsafe { (GetKeyState(VK_CONTROL as i32) < 0, GetKeyState(VK_SHIFT as i32) < 0) };
        if !ctrl {
            return false;
        }
        let key = msg.wParam as u16;
        match (key, shift) {
            (0x4E, false) => self.new_document(),
            (0x4F, false) => open_dialog(self.hwnd).iter().for_each(|path| self.open(path)),
            (0x53, false) => drop(self.save(false)),
            (0x53, true) => drop(self.save(true)),
            (0x57, false) => self.close(self.active.get()),
            (k, false) if k == VK_F4 => self.close(self.active.get()),
            (k, false) if k == VK_TAB => self.cycle(1),
            (k, true) if k == VK_TAB => self.cycle(-1),
            _ => return false,
        }
        true
    }

    fn layout(&self) {
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: finestre valide create da questo thread.
        unsafe {
            GetClientRect(self.hwnd, &mut client);
            let (top, bottom) = (self.scale(TAB_HEIGHT), self.scale(STATUS_HEIGHT));
            MoveWindow(self.sci.hwnd, 0, top, client.right, (client.bottom - top - bottom).max(0), 1);
        }
    }

    fn refresh(&self) {
        let title = {
            let docs = self.docs.borrow();
            docs.get(self.active.get()).map_or_else(String::new, |doc| format!("{}{} — Steroitor", if doc.dirty { "● " } else { "" }, doc.name()))
        };
        let title = wide(&title);
        // SAFETY: finestra valida, stringa terminata da zero.
        unsafe {
            SetWindowTextW(self.hwnd, title.as_ptr());
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
        }
    }

    fn on_notify(&self, notification: &Notification) {
        match notification.code {
            SCN_SAVEPOINTREACHED | SCN_SAVEPOINTLEFT => {
                if let Some(doc) = self.docs.borrow_mut().get_mut(self.active.get()) {
                    doc.dirty = notification.code == SCN_SAVEPOINTLEFT;
                }
                self.refresh();
            }
            SCN_UPDATEUI => {
                self.match_braces();
                // SAFETY: finestra valida.
                unsafe {
                    InvalidateRect(self.hwnd, &self.status_rect(), 0);
                }
            }
            _ => {}
        }
    }

    fn match_braces(&self) {
        let caret = self.sci.call(SCI_GETCURRENTPOS, 0, 0);
        let is_brace = |pos: isize| pos >= 0 && b"()[]{}".contains(&(self.sci.call(SCI_GETCHARAT, pos as usize, 0) as u8));
        let brace = if is_brace(caret - 1) { caret - 1 } else if is_brace(caret) { caret } else { -1 };
        if brace < 0 {
            self.sci.call(SCI_BRACEHIGHLIGHT, usize::MAX, -1);
            return;
        }
        let other = self.sci.call(SCI_BRACEMATCH, brace as usize, 0);
        if other < 0 {
            self.sci.call(SCI_BRACEBADLIGHT, brace as usize, 0);
        } else {
            self.sci.call(SCI_BRACEHIGHLIGHT, brace as usize, other);
        }
    }

    fn client_rect(&self) -> RECT {
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: finestra valida.
        unsafe {
            GetClientRect(self.hwnd, &mut client);
        }
        client
    }

    fn status_rect(&self) -> RECT {
        let client = self.client_rect();
        RECT { top: client.bottom - self.scale(STATUS_HEIGHT), ..client }
    }

    fn paint(&self) {
        // SAFETY: disegno GDI su un bitmap in memoria, poi copiato nel DC della finestra; gli oggetti creati vengono eliminati.
        unsafe {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let target = BeginPaint(self.hwnd, &mut paint);
            let client = self.client_rect();
            let dc = CreateCompatibleDC(target);
            let bitmap = CreateCompatibleBitmap(target, client.right.max(1), client.bottom.max(1));
            let previous_bitmap = SelectObject(dc, bitmap);
            let previous_font = SelectObject(dc, self.font.get());
            SetBkMode(dc, TRANSPARENT as i32);
            self.paint_tabs(dc, &client);
            self.paint_status(dc);
            let strip = self.scale(TAB_HEIGHT);
            BitBlt(target, 0, 0, client.right, strip, dc, 0, 0, SRCCOPY);
            let status = self.status_rect();
            BitBlt(target, 0, status.top, client.right, status.bottom - status.top, dc, 0, status.top, SRCCOPY);
            SelectObject(dc, previous_font);
            SelectObject(dc, previous_bitmap);
            DeleteObject(bitmap);
            DeleteDC(dc);
            EndPaint(self.hwnd, &paint);
        }
    }

    unsafe fn fill(dc: HDC, rect: &RECT, color: u32) {
        let brush = CreateSolidBrush(color);
        FillRect(dc, rect, brush);
        DeleteObject(brush);
    }

    unsafe fn text(dc: HDC, text: &str, rect: &RECT, color: u32, flags: u32) {
        let mut text: Vec<u16> = text.encode_utf16().collect();
        let mut rect = *rect;
        SetTextColor(dc, color);
        DrawTextW(dc, text.as_mut_ptr(), text.len() as i32, &mut rect, flags | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
    }

    unsafe fn text_width(dc: HDC, text: &str) -> i32 {
        let text: Vec<u16> = text.encode_utf16().collect();
        let mut size = std::mem::zeroed();
        GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut size);
        size.cx
    }

    unsafe fn paint_tabs(&self, dc: HDC, client: &RECT) {
        let palette = &self.palette;
        let height = self.scale(TAB_HEIGHT);
        Self::fill(dc, &RECT { bottom: height, ..*client }, palette.panel);
        Self::fill(dc, &RECT { top: height - 1, bottom: height, ..*client }, palette.border);
        let docs = self.docs.borrow();
        let mut rects = Vec::with_capacity(docs.len());
        let (padding, close) = (self.scale(TAB_PADDING), self.scale(CLOSE_SIZE));
        let mut left = 0;
        for (index, doc) in docs.iter().enumerate() {
            let name = doc.name();
            let width = (Self::text_width(dc, &name) + padding * 3 + close).clamp(self.scale(TAB_MIN_WIDTH), self.scale(TAB_MAX_WIDTH));
            let tab = RECT { left, top: 0, right: left + width, bottom: height };
            let active = index == self.active.get();
            if active {
                Self::fill(dc, &RECT { bottom: height, ..tab }, palette.editor);
                Self::fill(dc, &RECT { bottom: self.scale(2), ..tab }, palette.accent);
            }
            Self::fill(dc, &RECT { left: tab.right - 1, top: 0, right: tab.right, bottom: height - 1 }, palette.border);
            let label = RECT { left: tab.left + padding, right: tab.right - padding - close, ..tab };
            Self::text(dc, &name, &label, if active { palette.fg } else { palette.muted }, DT_LEFT | DT_END_ELLIPSIS);
            let close_rect = RECT { left: tab.right - padding - close, top: (height - close) / 2, right: tab.right - padding, bottom: (height + close) / 2 };
            Self::text(dc, if doc.dirty { "●" } else { "×" }, &close_rect, palette.muted, DT_CENTER);
            rects.push((tab, close_rect));
            left += width;
        }
        *self.tabs.borrow_mut() = rects;
    }

    unsafe fn paint_status(&self, dc: HDC) {
        let palette = &self.palette;
        let rect = self.status_rect();
        Self::fill(dc, &rect, palette.panel);
        Self::fill(dc, &RECT { bottom: rect.top + 1, ..rect }, palette.border);
        let docs = self.docs.borrow();
        let Some(doc) = docs.get(self.active.get()) else { return };
        let caret = self.sci.call(SCI_GETCURRENTPOS, 0, 0);
        let line = self.sci.call(SCI_LINEFROMPOSITION, caret as usize, 0) + 1;
        let column = self.sci.call(SCI_GETCOLUMN, caret as usize, 0) + 1;
        let right = format!(
            "Ln {line}, Col {column}     {}     {}{}     {}",
            eol_label(doc.eol),
            doc.encoding,
            if doc.bom { " BOM" } else { "" },
            doc.language.name
        );
        let padding = self.scale(TAB_PADDING);
        let inner = RECT { left: rect.left + padding, right: rect.right - padding, ..rect };
        Self::text(dc, &right, &inner, palette.muted, DT_RIGHT);
        let path = doc.path.as_ref().map_or_else(String::new, |path| path.display().to_string());
        let left = RECT { right: inner.right - Self::text_width(dc, &right) - padding, ..inner };
        Self::text(dc, &path, &left, palette.muted, DT_LEFT | DT_PATH_ELLIPSIS);
    }

    fn tab_at(&self, x: i32, y: i32) -> Option<(usize, bool)> {
        let inside = |rect: &RECT| x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
        self.tabs.borrow().iter().position(|(tab, _)| inside(tab)).map(|index| (index, inside(&self.tabs.borrow()[index].1)))
    }
}

fn dialog_filter() -> Vec<u16> {
    "Tutti i file (*.*)\0*.*\0\0".encode_utf16().collect()
}

fn open_dialog(owner: HWND) -> Vec<PathBuf> {
    let mut buffer = vec![0u16; 32 * 1024];
    let filter = dialog_filter();
    // SAFETY: buffer e filtro vivono per tutta la chiamata; nMaxFile ne riporta la lunghezza.
    let accepted = unsafe {
        let mut dialog: OPENFILENAMEW = std::mem::zeroed();
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.hwndOwner = owner;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = buffer.as_mut_ptr();
        dialog.nMaxFile = buffer.len() as u32;
        dialog.Flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_ALLOWMULTISELECT | OFN_NOCHANGEDIR;
        GetOpenFileNameW(&mut dialog) != 0
    };
    if !accepted {
        return Vec::new();
    }
    // Una selezione: il percorso completo. Più file: la cartella, poi i nomi, separati da \0.
    let parts: Vec<String> = buffer.split(|&unit| unit == 0).take_while(|part| !part.is_empty()).map(String::from_utf16_lossy).collect();
    match parts.as_slice() {
        [single] => vec![PathBuf::from(single)],
        [dir, names @ ..] => names.iter().map(|name| Path::new(dir).join(name)).collect(),
        [] => Vec::new(),
    }
}

fn save_dialog(owner: HWND, suggested: &str) -> Option<PathBuf> {
    let mut buffer = vec![0u16; 1024];
    for (slot, unit) in buffer.iter_mut().zip(suggested.encode_utf16().take(1000)) {
        *slot = unit;
    }
    let filter = dialog_filter();
    // SAFETY: come in open_dialog.
    let accepted = unsafe {
        let mut dialog: OPENFILENAMEW = std::mem::zeroed();
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.hwndOwner = owner;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = buffer.as_mut_ptr();
        dialog.nMaxFile = buffer.len() as u32;
        dialog.Flags = OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;
        GetSaveFileNameW(&mut dialog) != 0
    };
    accepted.then(|| PathBuf::from(String::from_utf16_lossy(buffer.split(|&unit| unit == 0).next().unwrap_or(&[]))))
}

fn point(lparam: LPARAM) -> (i32, i32) {
    ((lparam & 0xFFFF) as i16 as i32, ((lparam >> 16) & 0xFFFF) as i16 as i32)
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(app) = app() else { return DefWindowProcW(hwnd, message, wparam, lparam) };
    match message {
        WM_SIZE => {
            app.layout();
            0
        }
        WM_PAINT => {
            app.paint();
            0
        }
        WM_ERASEBKGND => 1,
        WM_SETFOCUS => {
            app.focus_editor();
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = point(lparam);
            match app.tab_at(x, y) {
                Some((index, true)) => app.close(index),
                Some((index, false)) => app.activate(index),
                None => {}
            }
            0
        }
        WM_MBUTTONUP => {
            let (x, y) = point(lparam);
            if let Some((index, _)) = app.tab_at(x, y) {
                app.close(index);
            }
            0
        }
        WM_NOTIFY => {
            let notification = &*(lparam as *const Notification);
            if notification.hwnd_from == app.sci.hwnd {
                app.on_notify(notification);
            }
            0
        }
        WM_DPICHANGED => {
            let suggested = &*(lparam as *const RECT);
            SetWindowPos(hwnd, std::ptr::null_mut(), suggested.left, suggested.top, suggested.right - suggested.left, suggested.bottom - suggested.top, SWP_NOZORDER | SWP_NOACTIVATE);
            app.update_font();
            app.layout();
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_CLOSE => {
            if app.confirm_quit() {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

//! Finestra principale: explorer a sinistra, tab, barra di ricerca ed editor Scintilla a destra, status bar in basso.
//! Tab, intestazioni e status bar sono disegnate a mano; c'è un documento Scintilla per tab.
use std::cell::{Cell, OnceCell, RefCell};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Controls::{SetWindowTheme, NMHDR};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use steroitor_core::text::{decode, encode};

use crate::dialogs::{message_box, open_files, pick_folder, save_file, wide};
use crate::explorer::{popup_menu, Action, Explorer};
use crate::find::{self, FindBar};
use crate::lang::{self, Language};
use crate::paint::{fill, text, text_width};
use crate::sci::*;
use crate::session::{EditorStyle, Session};
use crate::theme::{system_palette, Palette};

extern "C" {
    fn steroitor_create_lexer(name: *const std::ffi::c_char) -> *mut c_void;
}

const CLASS_NAME: &str = "SteroitorMain";
const TAB_HEIGHT: i32 = 32;
const STATUS_HEIGHT: i32 = 24;
const SPLITTER: i32 = 4;
const TAB_PADDING: i32 = 12;
const TAB_MIN_WIDTH: i32 = 80;
const TAB_MAX_WIDTH: i32 = 220;
const CLOSE_SIZE: i32 = 16;
const UI_FONT_PX: i32 = 13;
const TREE_ITEM_HEIGHT: i32 = 22;
const EXPLORER_WIDTH: i32 = 240;
const EXPLORER_MIN: i32 = 120;
const EDITOR_MIN: i32 = 200;
/// Il watcher (altro thread) avvisa la finestra con questo messaggio; le cartelle cambiate aspettano in `changes`.
const WM_FILES_CHANGED: u32 = WM_APP + 1;
const MENU_OPEN_FOLDER: usize = 1;
const MENU_CLOSE_FOLDER: usize = 2;

struct Doc {
    pointer: isize,
    path: Option<PathBuf>,
    untitled: u32,
    encoding: String,
    bom: bool,
    eol: usize,
    language: &'static Language,
    dirty: bool,
    /// Data di modifica del file quando è stato letto o salvato: dice se è cambiato su disco.
    modified: Option<SystemTime>,
    /// Cambiato su disco mentre non era la tab attiva: si ricarica quando la si apre.
    stale: bool,
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

fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

pub struct App {
    hwnd: HWND,
    sci: Sci,
    explorer: Explorer,
    find: FindBar,
    palette: Palette,
    style: EditorStyle,
    docs: RefCell<Vec<Doc>>,
    active: Cell<usize>,
    next_untitled: Cell<u32>,
    font: Cell<HFONT>,
    edit_brush: HBRUSH,
    tabs: RefCell<Vec<(RECT, RECT)>>,
    eol_rect: Cell<RECT>,
    explorer_width: Cell<i32>,
    explorer_visible: Cell<bool>,
    dragging: Cell<bool>,
    wrap: Cell<bool>,
    watcher: RefCell<Option<Debouncer<RecommendedWatcher>>>,
    changes: Arc<Mutex<Vec<String>>>,
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

fn contains(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

/// Menu contestuali scuri come il resto dell'app: API non documentata di uxtheme (ordinali 135 e 136), la usa anche Notepad++.
fn dark_menus() {
    // SAFETY: se gli ordinali non esistono GetProcAddress restituisce None e non si chiama nulla.
    unsafe {
        let name = wide("uxtheme.dll");
        let module = LoadLibraryW(name.as_ptr());
        if module.is_null() {
            return;
        }
        if let Some(set_mode) = GetProcAddress(module, 135 as *const u8) {
            let set_mode: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(set_mode);
            set_mode(2);
        }
        if let Some(flush) = GetProcAddress(module, 136 as *const u8) {
            let flush: unsafe extern "system" fn() = std::mem::transmute(flush);
            flush();
        }
    }
}

impl App {
    pub fn create(instance: HINSTANCE, arguments: Vec<PathBuf>) -> &'static App {
        let palette = system_palette();
        let session = Session::load();
        if palette.dark {
            dark_menus();
        }
        let class_name = wide(CLASS_NAME);
        // SAFETY: chiamate Win32 con strutture inizializzate e stringhe terminate da zero.
        let (hwnd, editor) = unsafe {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassExW(&class);
            let title = wide("Steroitor");
            let [left, top, right, bottom] = session.window.unwrap_or([CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT + 1200, CW_USEDEFAULT + 800]);
            let hwnd = CreateWindowExW(
                WS_EX_ACCEPTFILES,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                left,
                top,
                if session.window.is_some() { right - left } else { 1200 },
                if session.window.is_some() { bottom - top } else { 800 },
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            let dark: i32 = palette.dark.into();
            DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, (&dark as *const i32).cast(), 4);
            DragAcceptFiles(hwnd, 1);
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
            explorer: Explorer::create(hwnd, instance, &palette),
            find: FindBar::create(hwnd, instance),
            palette,
            style: EditorStyle::load(),
            docs: RefCell::new(Vec::new()),
            active: Cell::new(0),
            next_untitled: Cell::new(1),
            font: Cell::new(std::ptr::null_mut()),
            // SAFETY: pennello creato una volta e usato per tutta la vita dell'app.
            edit_brush: unsafe { CreateSolidBrush(palette.editor) },
            tabs: RefCell::new(Vec::new()),
            eol_rect: Cell::new(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
            explorer_width: Cell::new(session.explorer_width.unwrap_or(EXPLORER_WIDTH)),
            explorer_visible: Cell::new(session.explorer_visible.unwrap_or(true)),
            dragging: Cell::new(false),
            wrap: Cell::new(session.wrap),
            watcher: RefCell::new(None),
            changes: Arc::new(Mutex::new(Vec::new())),
        }));
        APP.with(|cell| cell.set(app).ok());
        app.configure_editor();
        app.update_font();

        let (folders, files): (Vec<PathBuf>, Vec<PathBuf>) = arguments.into_iter().partition(|path| path.is_dir());
        let restore = folders.is_empty() && files.is_empty();
        let folder = if restore { session.folder.clone().filter(|folder| folder.is_dir()) } else { folders.into_iter().next() };
        app.set_folder(folder);
        let to_open = if restore { session.files.clone() } else { files };
        for file in to_open.iter().filter(|file| file.is_file()) {
            app.open(file);
        }
        if app.docs.borrow().is_empty() {
            app.new_document();
        } else if restore {
            app.activate(session.active.min(app.docs.borrow().len() - 1));
        }
        app.layout();
        // SAFETY: finestra appena creata da questo thread.
        unsafe {
            ShowWindow(hwnd, if session.maximized { SW_SHOWMAXIMIZED } else { SW_SHOW });
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
            for control in [self.find.find, self.find.replace] {
                SendMessageW(control, WM_SETFONT, font as usize, 1);
            }
            self.explorer.set_font(font, self.scale(TREE_ITEM_HEIGHT));
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
        sci.call(SCI_SETWRAPMODE, if self.wrap.get() { SC_WRAP_WORD } else { 0 }, 0);
        let opaque = |color: u32| (color | 0xFF00_0000) as isize;
        let palette = &self.palette;
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_CARET, opaque(palette.accent));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_CARET_LINE_BACK, opaque(palette.active_line));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_BACK, opaque(palette.selection));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_ADDITIONAL_BACK, opaque(palette.selection));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_INACTIVE_BACK, opaque(palette.selection));
        sci.call(SCI_SETMARGINBACKN, 0, palette.editor as isize);
        sci.call(SCI_INDICSETSTYLE, find::INDICATOR, INDIC_ROUNDBOX);
        sci.call(SCI_INDICSETFORE, find::INDICATOR, palette.accent as isize);
        sci.call(SCI_INDICSETALPHA, find::INDICATOR, 60);
        sci.call(SCI_INDICSETOUTLINEALPHA, find::INDICATOR, 160);
        sci.call(SCI_INDICSETUNDER, find::INDICATOR, 1);
    }

    /// Gli stili sono della vista: si riapplicano a ogni cambio di tab, perché ogni lexer numera gli stili a modo suo.
    fn apply_styles(&self, language: &Language) {
        let sci = &self.sci;
        let palette = &self.palette;
        sci.call_str(SCI_STYLESETFONT, STYLE_DEFAULT, "Cascadia Mono");
        sci.call(SCI_STYLESETSIZEFRACTIONAL, STYLE_DEFAULT, self.style.font_size_fraction());
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
        let modified = path.as_deref().and_then(modified_time);
        // In lista prima di caricare il testo: le notifiche di modifica devono arrivare a questa tab, non alla precedente.
        let index = {
            let mut docs = self.docs.borrow_mut();
            docs.push(Doc { pointer, path, untitled, encoding, bom, eol, language, dirty: false, modified, stale: false, anchor: 0, caret: 0, first_line: 0 });
            docs.len() - 1
        };
        self.active.set(index);
        self.sci.call(SCI_SETCODEPAGE, SC_CP_UTF8, 0);
        self.sci.call(SCI_SETEOLMODE, eol, 0);
        self.sci.call(SCI_SETTABWIDTH, self.style.tab_size, 0);
        self.sci.call(SCI_SETINDENT, self.style.indent_size, 0);
        self.sci.call(SCI_SETUSETABS, self.style.use_tabs.into(), 0);
        self.set_language(language);
        self.apply_styles(language);
        self.sci.set_text(text.as_bytes());
        self.refresh();
    }

    pub fn new_document(&self) {
        self.add_document(None, "", "UTF-8".into(), false);
    }

    fn read(&self, path: &Path) -> Option<steroitor_core::text::TextFile> {
        match std::fs::read(path) {
            Ok(bytes) => Some(decode(&bytes)),
            Err(error) => {
                message_box(self.hwnd, &format!("Impossibile aprire {}:\n{error}", path.display()), MB_ICONERROR);
                None
            }
        }
    }

    pub fn open(&self, path: &Path) {
        let existing = self.docs.borrow().iter().position(|doc| doc.path.as_deref() == Some(path));
        if let Some(index) = existing {
            return self.activate(index);
        }
        let Some(file) = self.read(path) else { return };
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

    /// Ricarica la tab attiva dal disco, tenendo il cursore e lo scroll.
    fn reload_active(&self) {
        let Some(path) = self.docs.borrow()[self.active.get()].path.clone() else { return };
        let Ok(bytes) = std::fs::read(&path) else { return };
        let file = decode(&bytes);
        let (anchor, caret, first_line) = (self.sci.call(SCI_GETANCHOR, 0, 0), self.sci.call(SCI_GETCURRENTPOS, 0, 0), self.sci.call(SCI_GETFIRSTVISIBLELINE, 0, 0));
        {
            let mut docs = self.docs.borrow_mut();
            let doc = &mut docs[self.active.get()];
            (doc.encoding, doc.bom, doc.stale, doc.modified) = (file.encoding, file.bom, false, modified_time(&path));
        }
        self.sci.set_text(file.content.as_bytes());
        let length = self.sci.call(SCI_GETLENGTH, 0, 0);
        self.sci.call(SCI_SETSEL, anchor.min(length) as usize, caret.min(length));
        self.sci.call(SCI_SETFIRSTVISIBLELINE, first_line as usize, 0);
        self.refresh();
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
        let (pointer, language, anchor, caret, first_line, stale) = {
            let doc = &self.docs.borrow()[index];
            (doc.pointer, doc.language, doc.anchor, doc.caret, doc.first_line, doc.stale)
        };
        self.active.set(index);
        self.sci.call(SCI_SETDOCPOINTER, 0, pointer);
        self.apply_styles(language);
        self.sci.call(SCI_SETSEL, anchor as usize, caret);
        self.sci.call(SCI_SETFIRSTVISIBLELINE, first_line as usize, 0);
        if stale {
            self.reload_active();
        }
        if self.find.is_open() {
            self.find.highlight(&self.sci);
        }
        self.refresh();
        self.focus_editor();
    }

    /// Toglie la tab senza chiedere nulla; se era l'ultima ne apre una vuota.
    fn remove(&self, index: usize) {
        if self.docs.borrow().len() == 1 {
            self.new_document();
        }
        if index == self.active.get() {
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
            None => match save_file(self.hwnd, &name) {
                Some(path) => path,
                None => return false,
            },
        };
        let written = encode(&self.sci.text(), &encoding, bom).and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
        if let Err(error) = written {
            message_box(self.hwnd, &format!("Impossibile salvare {}:\n{error}", path.display()), MB_ICONERROR);
            return false;
        }
        let changed_path = current.as_deref() != Some(path.as_path());
        let language = lang::detect(&path);
        {
            let mut docs = self.docs.borrow_mut();
            docs[index].modified = modified_time(&path);
            if changed_path {
                docs[index].path = Some(path);
                docs[index].language = language;
            }
        }
        if changed_path {
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

    fn set_folder(&self, folder: Option<PathBuf>) {
        self.watcher.borrow_mut().take();
        self.explorer.set_root(folder.as_deref());
        if let Some(folder) = &folder {
            let changes = Arc::clone(&self.changes);
            let target = self.hwnd as usize;
            let watcher = steroitor_core::watch::watch(folder, move |_, dirs| {
                if let Ok(mut pending) = changes.lock() {
                    pending.extend(dirs);
                }
                // SAFETY: PostMessageW è sicura da qualsiasi thread; se la finestra non esiste più non fa nulla.
                unsafe { PostMessageW(target as HWND, WM_FILES_CHANGED, 0, 0) };
            });
            *self.watcher.borrow_mut() = watcher.ok();
            self.explorer_visible.set(true);
        }
        self.layout();
        self.refresh();
    }

    fn open_folder(&self) {
        if let Some(folder) = pick_folder(self.hwnd) {
            self.set_folder(Some(folder));
        }
    }

    fn on_files_changed(&self) {
        let dirs: Vec<String> = self.changes.lock().map(|mut pending| std::mem::take(&mut *pending)).unwrap_or_default();
        for dir in &dirs {
            self.explorer.refresh_dir(Path::new(dir));
        }
        let changed: Vec<usize> = self
            .docs
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, doc)| !doc.dirty)
            .filter(|(_, doc)| doc.path.as_deref().is_some_and(|path| dirs.iter().any(|dir| path.parent() == Some(Path::new(dir))) && modified_time(path).is_some_and(|time| Some(time) != doc.modified)))
            .map(|(index, _)| index)
            .collect();
        for index in changed {
            if index == self.active.get() {
                self.reload_active();
            } else {
                self.docs.borrow_mut()[index].stale = true;
            }
        }
    }

    fn on_explorer_action(&self, action: Action) {
        match action {
            Action::Open(path) => self.open(&path),
            Action::Renamed { from, to } => {
                let renamed: Vec<(usize, PathBuf)> = self
                    .docs
                    .borrow()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, doc)| doc.path.as_ref()?.strip_prefix(&from).ok().map(|rest| (index, if rest.as_os_str().is_empty() { to.clone() } else { to.join(rest) })))
                    .collect();
                for (index, path) in renamed {
                    let language = lang::detect(&path);
                    {
                        let mut docs = self.docs.borrow_mut();
                        docs[index].path = Some(path);
                        docs[index].language = language;
                    }
                    if index == self.active.get() {
                        self.set_language(language);
                        self.apply_styles(language);
                    }
                }
                self.refresh();
            }
            Action::Deleted(path) => {
                // Le tab dei file eliminati si chiudono, tranne quelle con modifiche: si possono ancora salvare.
                while let Some(index) = self.docs.borrow().iter().position(|doc| !doc.dirty && doc.path.as_deref().is_some_and(|file| file.starts_with(&path))) {
                    self.remove(index);
                }
            }
        }
    }

    fn toggle_eol(&self) {
        let eol = if self.docs.borrow()[self.active.get()].eol == SC_EOL_CRLF { SC_EOL_LF } else { SC_EOL_CRLF };
        self.docs.borrow_mut()[self.active.get()].eol = eol;
        self.sci.call(SCI_SETEOLMODE, eol, 0);
        self.sci.call(SCI_CONVERTEOLS, eol, 0);
        self.refresh();
    }

    fn toggle_explorer(&self) {
        self.explorer_visible.set(!self.explorer_visible.get());
        self.layout();
        if self.explorer_visible.get() {
            // SAFETY: controllo valido.
            unsafe { SetFocus(self.explorer.hwnd) };
        } else {
            self.focus_editor();
        }
    }

    fn toggle_wrap(&self) {
        self.wrap.set(!self.wrap.get());
        self.sci.call(SCI_SETWRAPMODE, if self.wrap.get() { SC_WRAP_WORD } else { 0 }, 0);
    }

    fn open_find(&self, replacing: bool) {
        let selection = {
            let length = self.sci.call(SCI_GETSELECTIONEND, 0, 0) - self.sci.call(SCI_GETSELECTIONSTART, 0, 0);
            let mut buffer = vec![0u8; length.max(0) as usize + 1];
            self.sci.call(SCI_GETSELTEXT, 0, buffer.as_mut_ptr() as isize);
            buffer.truncate(length.max(0) as usize);
            String::from_utf8_lossy(&buffer).into_owned()
        };
        self.find.show(replacing, Some(selection));
        self.find.highlight(&self.sci);
        self.layout();
    }

    fn close_find(&self) {
        self.find.hide(&self.sci);
        self.layout();
        self.focus_editor();
    }

    fn on_find_button(&self, button: find::Button) {
        match button {
            find::Button::Previous => self.find.search(&self.sci, false, false),
            find::Button::Next => self.find.search(&self.sci, true, false),
            find::Button::MatchCase => self.find.toggle_match_case(&self.sci),
            find::Button::ReplaceOne => self.find.replace_one(&self.sci),
            find::Button::ReplaceAll => self.find.replace_all(&self.sci),
            find::Button::Close => return self.close_find(),
        }
        self.invalidate();
    }

    /// Scorciatoie dell'app, prima che il tasto arrivi al controllo con il focus. true se il tasto è stato usato.
    pub fn shortcut(&self, msg: &MSG) -> bool {
        // SAFETY: lettura dello stato della tastiera e del focus del thread.
        let (ctrl, shift, focus) = unsafe { (GetKeyState(VK_CONTROL as i32) < 0, GetKeyState(VK_SHIFT as i32) < 0, GetFocus()) };
        let key = msg.wParam as u16;
        if msg.message == WM_SYSKEYDOWN {
            match key {
                0x31 => self.toggle_explorer(),
                0x5A => self.toggle_wrap(),
                _ => return false,
            }
            return true;
        }
        if msg.message != WM_KEYDOWN {
            return false;
        }
        if self.find.owns(focus) && !ctrl {
            match key {
                VK_RETURN if focus == self.find.replace => self.find.replace_one(&self.sci),
                VK_RETURN => self.find.search(&self.sci, !shift, false),
                VK_ESCAPE => self.close_find(),
                VK_TAB => {
                    let other = if focus == self.find.find { self.find.replace } else { self.find.find };
                    // SAFETY: controllo valido.
                    unsafe { SetFocus(other) };
                }
                _ => return false,
            }
            self.invalidate();
            return true;
        }
        if focus == self.explorer.hwnd && !ctrl {
            match key {
                VK_F2 => self.explorer.rename_selected(),
                VK_DELETE => {
                    if let Some(action) = self.explorer.delete_selected(self.hwnd) {
                        self.on_explorer_action(action);
                    }
                }
                _ => return false,
            }
            return true;
        }
        if key == VK_F3 && !ctrl {
            self.find.search(&self.sci, !shift, false);
            self.invalidate();
            return true;
        }
        if !ctrl {
            return false;
        }
        match (key, shift) {
            (0x4E, false) => self.new_document(),
            (0x4F, false) => open_files(self.hwnd).iter().for_each(|path| self.open(path)),
            (0x53, false) => drop(self.save(false)),
            (0x53, true) => drop(self.save(true)),
            (0x57, false) | (VK_F4, false) => self.close(self.active.get()),
            (VK_TAB, false) => self.cycle(1),
            (VK_TAB, true) => self.cycle(-1),
            (0x46, false) => self.open_find(false),
            (0x48, false) => self.open_find(true),
            _ => return false,
        }
        true
    }

    /// Bordo destro dell'explorer in pixel reali (0 se nascosto); la larghezza è salvata in pixel a 96 DPI.
    fn explorer_edge(&self) -> i32 {
        if self.explorer_visible.get() {
            self.scale(self.explorer_width.get().max(EXPLORER_MIN))
        } else {
            0
        }
    }

    fn editor_left(&self) -> i32 {
        let edge = self.explorer_edge();
        if edge == 0 {
            0
        } else {
            edge + self.scale(SPLITTER)
        }
    }

    fn layout(&self) {
        let client = self.client_rect();
        let (tab, status) = (self.scale(TAB_HEIGHT), self.scale(STATUS_HEIGHT));
        let bottom = client.bottom - status;
        let left = self.editor_left();
        let find_height = self.scale(self.find.height());
        // SAFETY: finestre valide create da questo thread.
        unsafe {
            ShowWindow(self.explorer.hwnd, if self.explorer_visible.get() { SW_SHOW } else { SW_HIDE });
            MoveWindow(self.explorer.hwnd, 0, tab, self.explorer_edge(), (bottom - tab).max(0), 1);
            MoveWindow(self.sci.hwnd, left, tab + find_height, (client.right - left).max(0), (bottom - tab - find_height).max(0), 1);
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
        }
        self.find.layout(RECT { left, top: tab, right: client.right, bottom: tab + find_height }, |value| self.scale(value));
    }

    fn invalidate(&self) {
        // SAFETY: finestra valida.
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    fn refresh(&self) {
        let title = {
            let docs = self.docs.borrow();
            let folder = self.explorer.root().and_then(|root| root.file_name().map(|name| format!(" — {}", name.to_string_lossy()))).unwrap_or_default();
            docs.get(self.active.get()).map_or_else(String::new, |doc| format!("{}{}{folder} — Steroitor", if doc.dirty { "● " } else { "" }, doc.name()))
        };
        let title = wide(&title);
        // SAFETY: finestra valida, stringa terminata da zero.
        unsafe { SetWindowTextW(self.hwnd, title.as_ptr()) };
        self.invalidate();
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
                unsafe { InvalidateRect(self.hwnd, &self.status_rect(), 0) };
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
        unsafe { GetClientRect(self.hwnd, &mut client) };
        client
    }

    fn status_rect(&self) -> RECT {
        let client = self.client_rect();
        RECT { top: client.bottom - self.scale(STATUS_HEIGHT), ..client }
    }

    fn splitter_rect(&self) -> Option<RECT> {
        let edge = self.explorer_edge();
        (edge > 0).then(|| RECT { left: edge, top: 0, right: edge + self.scale(SPLITTER), bottom: self.status_rect().top })
    }

    fn header_rect(&self) -> RECT {
        RECT { left: 0, top: 0, right: self.explorer_edge(), bottom: self.scale(TAB_HEIGHT) }
    }

    fn find_rect(&self) -> RECT {
        let top = self.scale(TAB_HEIGHT);
        RECT { left: self.editor_left(), top, right: self.client_rect().right, bottom: top + self.scale(self.find.height()) }
    }

    fn paint(&self) {
        // SAFETY: disegno GDI su un bitmap in memoria, poi copiato; WS_CLIPCHILDREN protegge i controlli figli.
        unsafe {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let target = BeginPaint(self.hwnd, &mut paint);
            let client = self.client_rect();
            let dc = CreateCompatibleDC(target);
            let bitmap = CreateCompatibleBitmap(target, client.right.max(1), client.bottom.max(1));
            let previous_bitmap = SelectObject(dc, bitmap);
            let previous_font = SelectObject(dc, self.font.get());
            SetBkMode(dc, TRANSPARENT as i32);
            fill(dc, &client, self.palette.panel);
            if self.explorer_visible.get() {
                self.paint_header(dc);
            }
            if let Some(splitter) = self.splitter_rect() {
                fill(dc, &splitter, self.palette.panel);
                fill(dc, &RECT { left: splitter.left, right: splitter.left + 1, ..splitter }, self.palette.border);
            }
            self.paint_tabs(dc, &client);
            if self.find.is_open() {
                self.find.paint(dc, self.find_rect(), &self.palette, |value| self.scale(value));
            }
            self.paint_status(dc);
            BitBlt(target, 0, 0, client.right, client.bottom, dc, 0, 0, SRCCOPY);
            SelectObject(dc, previous_font);
            SelectObject(dc, previous_bitmap);
            DeleteObject(bitmap);
            DeleteDC(dc);
            EndPaint(self.hwnd, &paint);
        }
    }

    unsafe fn paint_header(&self, dc: HDC) {
        let rect = self.header_rect();
        fill(dc, &RECT { top: rect.bottom - 1, ..rect }, self.palette.border);
        let inner = RECT { left: rect.left + self.scale(TAB_PADDING), right: rect.right - self.scale(TAB_PADDING), ..rect };
        match self.explorer.root() {
            Some(root) => {
                let name = root.file_name().map_or_else(|| root.display().to_string(), |name| name.to_string_lossy().into_owned());
                text(dc, &name, &inner, self.palette.fg, DT_LEFT | DT_END_ELLIPSIS);
            }
            None => text(dc, "Apri cartella…", &inner, self.palette.accent, DT_LEFT | DT_END_ELLIPSIS),
        }
    }

    unsafe fn paint_tabs(&self, dc: HDC, client: &RECT) {
        let palette = &self.palette;
        let height = self.scale(TAB_HEIGHT);
        let start = self.editor_left();
        Self::strip(dc, RECT { left: start, bottom: height, ..*client }, palette);
        let docs = self.docs.borrow();
        let mut rects = Vec::with_capacity(docs.len());
        let (padding, close) = (self.scale(TAB_PADDING), self.scale(CLOSE_SIZE));
        let mut left = start;
        for (index, doc) in docs.iter().enumerate() {
            let name = doc.name();
            let width = (text_width(dc, &name) + padding * 3 + close).clamp(self.scale(TAB_MIN_WIDTH), self.scale(TAB_MAX_WIDTH));
            let tab = RECT { left, top: 0, right: left + width, bottom: height };
            let active = index == self.active.get();
            if active {
                fill(dc, &tab, palette.editor);
                fill(dc, &RECT { bottom: self.scale(2), ..tab }, palette.accent);
            }
            fill(dc, &RECT { left: tab.right - 1, top: 0, right: tab.right, bottom: height - 1 }, palette.border);
            let label = RECT { left: tab.left + padding, right: tab.right - padding - close, ..tab };
            text(dc, &name, &label, if active { palette.fg } else { palette.muted }, DT_LEFT | DT_END_ELLIPSIS);
            let close_rect = RECT { left: tab.right - padding - close, top: (height - close) / 2, right: tab.right - padding, bottom: (height + close) / 2 };
            text(dc, if doc.dirty { "●" } else { "×" }, &close_rect, palette.muted, DT_CENTER);
            rects.push((tab, close_rect));
            left += width;
        }
        *self.tabs.borrow_mut() = rects;
    }

    unsafe fn strip(dc: HDC, rect: RECT, palette: &Palette) {
        fill(dc, &rect, palette.panel);
        fill(dc, &RECT { top: rect.bottom - 1, ..rect }, palette.border);
    }

    unsafe fn paint_status(&self, dc: HDC) {
        let palette = &self.palette;
        let rect = self.status_rect();
        fill(dc, &rect, palette.panel);
        fill(dc, &RECT { bottom: rect.top + 1, ..rect }, palette.border);
        let docs = self.docs.borrow();
        let Some(doc) = docs.get(self.active.get()) else { return };
        let caret = self.sci.call(SCI_GETCURRENTPOS, 0, 0);
        let line = self.sci.call(SCI_LINEFROMPOSITION, caret as usize, 0) + 1;
        let column = self.sci.call(SCI_GETCOLUMN, caret as usize, 0) + 1;
        let padding = self.scale(TAB_PADDING);
        // Da destra a sinistra: linguaggio, encoding, fine riga (cliccabile), posizione.
        let segments = [doc.language.name.to_string(), format!("{}{}", doc.encoding, if doc.bom { " BOM" } else { "" }), eol_label(doc.eol).to_string(), format!("Ln {line}, Col {column}")];
        let mut right = rect.right - padding;
        for (index, segment) in segments.iter().enumerate() {
            let width = text_width(dc, segment);
            let area = RECT { left: right - width, right, ..rect };
            text(dc, segment, &area, palette.muted, DT_LEFT);
            if index == 2 {
                self.eol_rect.set(RECT { left: area.left - padding / 2, right: area.right + padding / 2, ..area });
            }
            right -= width + padding * 2;
        }
        let path = doc.path.as_ref().map_or_else(String::new, |path| path.display().to_string());
        let left = RECT { left: rect.left + padding, right, ..rect };
        text(dc, &path, &left, palette.muted, DT_LEFT | DT_PATH_ELLIPSIS);
    }

    fn tab_at(&self, x: i32, y: i32) -> Option<(usize, bool)> {
        let tabs = self.tabs.borrow();
        let index = tabs.iter().position(|(tab, _)| contains(tab, x, y))?;
        Some((index, contains(&tabs[index].1, x, y)))
    }

    fn on_click(&self, x: i32, y: i32) {
        if self.splitter_rect().is_some_and(|splitter| contains(&splitter, x, y)) {
            self.dragging.set(true);
            // SAFETY: cattura del mouse sulla finestra principale fino al rilascio.
            unsafe { SetCapture(self.hwnd) };
            return;
        }
        if self.explorer_visible.get() && contains(&self.header_rect(), x, y) {
            return self.header_menu();
        }
        if self.find.is_open() && contains(&self.find_rect(), x, y) {
            if let Some(button) = self.find.button_at(x, y) {
                self.on_find_button(button);
            }
            return;
        }
        if contains(&self.eol_rect.get(), x, y) {
            return self.toggle_eol();
        }
        match self.tab_at(x, y) {
            Some((index, true)) => self.close(index),
            Some((index, false)) => self.activate(index),
            None => {}
        }
    }

    fn header_menu(&self) {
        if self.explorer.root().is_none() {
            return self.open_folder();
        }
        match popup_menu(self.hwnd, &[(MENU_OPEN_FOLDER, "Apri cartella…"), (MENU_CLOSE_FOLDER, "Chiudi cartella")]) {
            MENU_OPEN_FOLDER => self.open_folder(),
            MENU_CLOSE_FOLDER => self.set_folder(None),
            _ => {}
        }
    }

    fn on_mouse_move(&self, x: i32) {
        if !self.dragging.get() {
            return;
        }
        let client = self.client_rect();
        let max = (client.right - self.scale(EDITOR_MIN)).max(self.scale(EXPLORER_MIN));
        let width = x.clamp(self.scale(EXPLORER_MIN), max);
        self.explorer_width.set(width * 96 / self.dpi().max(1));
        self.layout();
    }

    fn on_drop(&self, drop: HDROP) {
        // SAFETY: HDROP valido fino a DragFinish; il buffer ha la lunghezza richiesta più il terminatore.
        let paths: Vec<PathBuf> = unsafe {
            let count = DragQueryFileW(drop, u32::MAX, std::ptr::null_mut(), 0);
            let paths = (0..count)
                .map(|index| {
                    let length = DragQueryFileW(drop, index, std::ptr::null_mut(), 0);
                    let mut buffer = vec![0u16; length as usize + 1];
                    DragQueryFileW(drop, index, buffer.as_mut_ptr(), buffer.len() as u32);
                    PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]))
                })
                .collect();
            DragFinish(drop);
            paths
        };
        for path in paths {
            if path.is_dir() {
                self.set_folder(Some(path));
            } else {
                self.open(&path);
            }
        }
    }

    fn save_session(&self) {
        let docs = self.docs.borrow();
        let saved: Vec<(usize, PathBuf)> = docs.iter().enumerate().filter_map(|(index, doc)| doc.path.clone().map(|path| (index, path))).collect();
        let active = saved.iter().position(|(index, _)| *index == self.active.get()).unwrap_or(0);
        // SAFETY: lettura della posizione della finestra.
        let (window, maximized) = unsafe {
            let mut placement: WINDOWPLACEMENT = std::mem::zeroed();
            placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
            GetWindowPlacement(self.hwnd, &mut placement);
            let normal = placement.rcNormalPosition;
            (Some([normal.left, normal.top, normal.right, normal.bottom]), IsZoomed(self.hwnd) != 0)
        };
        Session {
            folder: self.explorer.root(),
            files: saved.into_iter().map(|(_, path)| path).collect(),
            active,
            explorer_width: Some(self.explorer_width.get()),
            explorer_visible: Some(self.explorer_visible.get()),
            window,
            maximized,
            wrap: self.wrap.get(),
        }
        .save();
    }

    fn set_cursor(&self) -> bool {
        let mut point = POINT { x: 0, y: 0 };
        // SAFETY: posizione del cursore convertita in coordinate client della finestra principale.
        unsafe {
            GetCursorPos(&mut point);
            ScreenToClient(self.hwnd, &mut point);
        }
        let over_splitter = self.splitter_rect().is_some_and(|splitter| contains(&splitter, point.x, point.y));
        let clickable = contains(&self.eol_rect.get(), point.x, point.y) || (self.explorer_visible.get() && contains(&self.header_rect(), point.x, point.y));
        let cursor = match (over_splitter || self.dragging.get(), clickable) {
            (true, _) => IDC_SIZEWE,
            (false, true) => IDC_HAND,
            _ => return false,
        };
        // SAFETY: cursore di sistema.
        unsafe { SetCursor(LoadCursorW(std::ptr::null_mut(), cursor)) };
        true
    }
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
        WM_SETCURSOR if (lparam & 0xFFFF) as u32 == HTCLIENT && wparam as HWND == hwnd => {
            if app.set_cursor() {
                1
            } else {
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
        }
        WM_LBUTTONDOWN => {
            let (x, y) = point(lparam);
            app.on_click(x, y);
            0
        }
        WM_MOUSEMOVE => {
            app.on_mouse_move(point(lparam).0);
            0
        }
        WM_LBUTTONUP => {
            if app.dragging.replace(false) {
                ReleaseCapture();
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
            let header = &*(lparam as *const NMHDR);
            if header.hwndFrom == app.sci.hwnd {
                app.on_notify(&*(lparam as *const Notification));
                return 0;
            }
            if header.hwndFrom == app.explorer.hwnd {
                let (result, action) = app.explorer.on_notify(hwnd, header, lparam);
                if let Some(action) = action {
                    app.on_explorer_action(action);
                }
                return result;
            }
            0
        }
        WM_COMMAND => {
            let notification = ((wparam >> 16) & 0xFFFF) as u32;
            if lparam as HWND == app.find.find && notification == EN_CHANGE {
                app.find.search(&app.sci, true, true);
                app.invalidate();
            }
            0
        }
        WM_CTLCOLOREDIT => {
            let dc = wparam as HDC;
            SetTextColor(dc, app.palette.fg);
            SetBkColor(dc, app.palette.editor);
            app.edit_brush as LRESULT
        }
        WM_DROPFILES => {
            app.on_drop(wparam as HDROP);
            0
        }
        WM_FILES_CHANGED => {
            app.on_files_changed();
            0
        }
        WM_DPICHANGED => {
            let suggested = &*(lparam as *const RECT);
            SetWindowPos(hwnd, std::ptr::null_mut(), suggested.left, suggested.top, suggested.right - suggested.left, suggested.bottom - suggested.top, SWP_NOZORDER | SWP_NOACTIVATE);
            app.update_font();
            app.layout();
            0
        }
        WM_CLOSE => {
            if app.confirm_quit() {
                app.save_session();
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

//! Finestra principale: explorer a sinistra, tab, barra di ricerca ed editor Scintilla a destra, status bar in basso.
//! Tab, intestazioni e status bar sono disegnate a mano; c'è un documento Scintilla per tab.
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
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
use windows_sys::Win32::UI::Controls::{SetWindowTheme, CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDRF_DODEFAULT, CDRF_NOTIFYITEMDRAW, NMHDR, NMTVCUSTOMDRAW, NM_CUSTOMDRAW};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use steroitor_core::git::history::ResetMode;
use steroitor_core::git::parse::FileStatus;
use steroitor_core::text::{decode, encode};

use crate::dialogs::{copy_to_clipboard, message_box, open_files, pick_folder, save_file, wide};
use crate::explorer::{popup_menu, Action, Explorer};
use crate::find::{self, window_text, FindBar};
use crate::git::{line_changes, repo_of, Event, FileColors, Repo, Worker, WM_COMMAND_LOGGED, WM_GIT_EVENT};
use crate::lang::{self, Language};
use crate::list::{decode_event, ListEvent, RowPaint, WM_LIST_EVENT, WM_LIST_PAINT};
use crate::paint::{fill, text, text_width};
use crate::panel::{self, Panel, Tab};
use crate::prompt::prompt;
use crate::sci::*;
use crate::session::{Session, Settings};
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
const MARKER_TIMER: usize = 1;
const MARKER_DELAY_MS: u32 = 300;
const GIT_MARGIN: usize = 1;
const MARKER_ADDED: usize = 0;
const MARKER_MODIFIED: usize = 1;
const MARKER_DELETED: usize = 2;
const LOG_CHECKOUT: usize = 1;
const LOG_BRANCH: usize = 2;
const LOG_TAG: usize = 3;
const LOG_CHERRY_PICK: usize = 4;
const LOG_REVERT: usize = 5;
const LOG_RESET_SOFT: usize = 6;
const LOG_RESET_MIXED: usize = 7;
const LOG_RESET_HARD: usize = 8;
const LOG_COPY_HASH: usize = 9;
const LIST_ROW_HEIGHT: i32 = 24;
const VCS_PULL: usize = 1;
const VCS_PUSH: usize = 2;
const VCS_FETCH: usize = 3;
const VCS_NEW_BRANCH: usize = 4;
const VCS_CHECKOUT: usize = 1000;
const VCS_MERGE: usize = 3000;

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
    panel: Panel,
    dragging_panel: Cell<bool>,
    find: FindBar,
    palette: Palette,
    settings: Settings,
    worker: Worker,
    repos: RefCell<Vec<Repo>>,
    colors: RefCell<FileColors>,
    /// Contenuto in HEAD dei file aperti (None = non tracciato), per i marker nel gutter.
    heads: RefCell<HashMap<String, Option<String>>>,
    vcs_rect: Cell<RECT>,
    head_requests: RefCell<std::collections::HashSet<String>>,
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
    /// Dal watcher: cartelle cambiate e repository con metadati Git cambiati.
    changes: Arc<Mutex<(Vec<String>, Vec<PathBuf>)>>,
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
            panel: Panel::create(hwnd, instance, &palette),
            dragging_panel: Cell::new(false),
            find: FindBar::create(hwnd, instance),
            palette,
            settings: Settings::load(),
            worker: Worker::new(hwnd),
            repos: RefCell::new(Vec::new()),
            colors: RefCell::new(FileColors::default()),
            heads: RefCell::new(HashMap::new()),
            vcs_rect: Cell::new(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
            head_requests: RefCell::default(),
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
            changes: Arc::default(),
        }));
        APP.with(|cell| cell.set(app).ok());
        app.panel.attach();
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
            SendMessageW(self.panel.message, WM_SETFONT, font as usize, 1);
            self.panel.log.set_row_height(self.scale(LIST_ROW_HEIGHT));
            self.panel.files.set_row_height(self.scale(LIST_ROW_HEIGHT));
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
        sci.call(SCI_SETMODEVENTMASK, (SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT) as usize, 0);
        sci.call(SCI_SETMARGINTYPEN, GIT_MARGIN, SC_MARGIN_SYMBOL);
        sci.call(SCI_SETMARGINMASKN, GIT_MARGIN, 0b111);
        sci.call(SCI_SETMARGINWIDTHN, GIT_MARGIN, self.scale(4) as isize);
        for (marker, color) in [(MARKER_ADDED, self.palette.git_added), (MARKER_MODIFIED, self.palette.git_modified), (MARKER_DELETED, self.palette.git_ignored)] {
            sci.call(SCI_MARKERDEFINE, marker, SC_MARK_LEFTRECT);
            sci.call(SCI_MARKERSETBACK, marker, color as isize);
        }
        sci.call(SCI_SETWRAPMODE, if self.wrap.get() { SC_WRAP_WORD } else { 0 }, 0);
        let opaque = |color: u32| (color | 0xFF00_0000) as isize;
        let palette = &self.palette;
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_CARET, opaque(palette.accent));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_CARET_LINE_BACK, opaque(palette.active_line));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_BACK, opaque(palette.selection));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_ADDITIONAL_BACK, opaque(palette.selection));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_INACTIVE_BACK, opaque(palette.selection));
        sci.call(SCI_SETMARGINBACKN, 0, palette.editor as isize);
        sci.call(SCI_SETMARGINBACKN, GIT_MARGIN, palette.editor as isize);
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
        sci.call(SCI_STYLESETSIZEFRACTIONAL, STYLE_DEFAULT, self.settings.editor.font_size_fraction());
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
        self.sci.call(SCI_SETTABWIDTH, self.settings.editor.tab_size, 0);
        self.sci.call(SCI_SETINDENT, self.settings.editor.indent_size, 0);
        self.sci.call(SCI_SETUSETABS, self.settings.editor.use_tabs.into(), 0);
        self.set_language(language);
        self.apply_styles(language);
        self.sci.set_text(text.as_bytes());
        self.refresh();
        self.update_markers();
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
        self.update_markers();
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
        let roots = folder.as_deref().map(steroitor_core::repos::find_repos).unwrap_or_default();
        *self.repos.borrow_mut() = roots.iter().map(|root| Repo { root: root.clone(), status: None, branches: Vec::new() }).collect();
        *self.colors.borrow_mut() = FileColors::default();
        self.heads.borrow_mut().clear();
        for root in &roots {
            self.worker.refresh(root);
        }
        if let Some(folder) = &folder {
            let changes = Arc::clone(&self.changes);
            let target = self.hwnd as usize;
            let watcher = steroitor_core::watch::watch(folder, move |repos, dirs| {
                if let Ok(mut pending) = changes.lock() {
                    pending.0.extend(dirs);
                    pending.1.extend(repos);
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
        let (dirs, git_dirs) = self.changes.lock().map(|mut pending| std::mem::take(&mut *pending)).unwrap_or_default();
        let touched: Vec<PathBuf> = {
            let repos = self.repos.borrow();
            let changed = dirs.iter().map(PathBuf::from).chain(git_dirs);
            let mut roots: Vec<PathBuf> = changed.filter_map(|path| repo_of(&repos, &path).map(|repo| repo.root.clone())).collect();
            roots.sort();
            roots.dedup();
            roots
        };
        for root in touched {
            self.worker.refresh(&root);
        }
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
                0xC0 => self.vcs_menu(),
                0x39 => self.toggle_panel(Tab::Git),
                _ => return false,
            }
            return true;
        }
        if msg.message != WM_KEYDOWN {
            return false;
        }
        // SAFETY: lettura dello stato della tastiera del thread.
        let alt = unsafe { GetKeyState(VK_MENU as i32) < 0 };
        if ctrl && alt && key == 0x4B {
            self.commit(true);
            return true;
        }
        if focus == self.panel.message && ctrl && key == VK_RETURN {
            self.commit(false);
            return true;
        }
        if ctrl && !shift && key == 0x54 && self.panel.owns(focus) {
            self.pull();
            return true;
        }
        if ctrl && !shift && key == 0x4B {
            self.show_panel(Tab::Commit);
            return true;
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
            (0x4B, true) => self.push(),
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
        let tab = self.scale(TAB_HEIGHT);
        let panel = self.panel_rect();
        let bottom = panel.top;
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
        self.panel.layout(panel, |value| self.scale(value));
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
            SCN_MODIFIED => {
                // SAFETY: timer della finestra principale; se esiste già viene solo riavviato.
                unsafe { SetTimer(self.hwnd, MARKER_TIMER, MARKER_DELAY_MS, None) };
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
        (edge > 0).then(|| RECT { left: edge, top: 0, right: edge + self.scale(SPLITTER), bottom: self.panel_rect().top })
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
            if self.panel.visible.get() {
                self.panel.paint(dc, self.panel_rect(), &self.palette, |value| self.scale(value));
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
        let mut left_edge = rect.left + padding;
        let branch = {
            let repos = self.repos.borrow();
            let repo = doc.path.as_deref().and_then(|path| repo_of(&repos, path)).or_else(|| repos.first());
            repo.and_then(|repo| repo.status.as_ref()).map(|status| {
                let branch = &status.branch;
                let name = branch.head.clone().unwrap_or_else(|| format!("HEAD staccato ({})", branch.oid.as_deref().and_then(|oid| oid.get(..7)).unwrap_or("?")));
                let ahead = if branch.ahead > 0 { format!(" ↑{}", branch.ahead) } else { String::new() };
                let behind = if branch.behind > 0 { format!(" ↓{}", branch.behind) } else { String::new() };
                format!("⎇ {name}{ahead}{behind}")
            })
        };
        match branch {
            Some(label) => {
                let width = text_width(dc, &label);
                let area = RECT { left: left_edge, right: left_edge + width, ..rect };
                text(dc, &label, &area, palette.fg, DT_LEFT);
                self.vcs_rect.set(RECT { left: area.left - padding / 2, right: area.right + padding / 2, ..area });
                left_edge = area.right + padding * 2;
            }
            None => self.vcs_rect.set(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
        }
        let path = doc.path.as_ref().map_or_else(String::new, |path| path.display().to_string());
        let left = RECT { left: left_edge, right, ..rect };
        text(dc, &path, &left, palette.muted, DT_LEFT | DT_PATH_ELLIPSIS);
    }

    fn active_path(&self) -> Option<PathBuf> {
        self.docs.borrow().get(self.active.get()).and_then(|doc| doc.path.clone())
    }

    /// Repository del file attivo, altrimenti il primo della cartella.
    fn active_repo_root(&self) -> Option<PathBuf> {
        let repos = self.repos.borrow();
        let of_file = self.active_path().and_then(|path| repo_of(&repos, &path).map(|repo| repo.root.clone()));
        of_file.or_else(|| repos.first().map(|repo| repo.root.clone()))
    }

    /// Marker nel gutter: righe aggiunte, modificate o tolte rispetto a HEAD. Il contenuto di HEAD si chiede una volta e resta in cache.
    fn update_markers(&self) {
        self.sci.call(SCI_MARKERDELETEALL, usize::MAX, 0);
        let Some(path) = self.active_path() else { return };
        let root = repo_of(&self.repos.borrow(), &path).map(|repo| repo.root.clone());
        let Some(root) = root else { return };
        let key = crate::git::key(&path);
        let head = self.heads.borrow().get(&key).cloned();
        let Some(head) = head else {
            if self.head_requests.borrow_mut().insert(key) {
                self.worker.head(&root, &path);
            }
            return;
        };
        let Some(head) = head else { return };
        let changes = line_changes(&head, &self.sci.text());
        for (lines, marker) in [(changes.added, MARKER_ADDED), (changes.modified, MARKER_MODIFIED), (changes.deleted, MARKER_DELETED)] {
            for line in lines {
                self.sci.call(SCI_MARKERADD, line, marker as isize);
            }
        }
    }

    fn on_git_events(&self) {
        for event in self.worker.take() {
            match event {
                Event::Status(root, Ok(status)) => {
                    let head_moved = {
                        let mut repos = self.repos.borrow_mut();
                        let Some(repo) = repos.iter_mut().find(|repo| repo.root == root) else { continue };
                        let moved = repo.status.as_ref().map(|old| old.branch.oid.clone()) != Some(status.branch.oid.clone());
                        repo.status = Some(status);
                        moved
                    };
                    *self.colors.borrow_mut() = FileColors::build(&self.repos.borrow());
                    if self.panel.root.borrow().as_deref() == Some(root.as_path()) {
                        if let Some(status) = self.repos.borrow().iter().find(|repo| repo.root == root).and_then(|repo| repo.status.as_ref()) {
                            self.panel.set_changes(&root, status);
                        }
                        if head_moved && self.panel.visible.get() {
                            self.worker.log(&root);
                        }
                    }
                    if head_moved {
                        // Commit, checkout, reset: il contenuto in HEAD dei file di questo repository va riletto.
                        let prefix = crate::git::key(&root);
                        self.heads.borrow_mut().retain(|key, _| !key.starts_with(&prefix));
                        self.update_markers();
                    }
                    // SAFETY: controllo valido; ridisegno per i nuovi colori.
                    unsafe { InvalidateRect(self.explorer.hwnd, std::ptr::null(), 0) };
                }
                Event::Status(_, Err(_)) => {}
                Event::Log(root, Ok(commits)) => {
                    if self.panel.root.borrow().as_deref() == Some(root.as_path()) {
                        self.panel.set_commits(commits);
                    }
                }
                Event::Log(_, Err(error)) => {
                    message_box(self.hwnd, &error, MB_ICONERROR);
                }
                Event::Committed(root, result) => {
                    match result {
                        Ok(()) => {
                            let empty = wide("");
                            // SAFETY: controllo valido, stringa terminata da zero.
                            unsafe { SetWindowTextW(self.panel.message, empty.as_ptr()) };
                        }
                        Err(error) => {
                            message_box(self.hwnd, &error, MB_ICONERROR);
                        }
                    }
                    self.worker.refresh(&root);
                    if self.panel.visible.get() {
                        self.worker.log(&root);
                    }
                }
                Event::Branches(root, branches) => {
                    if let Some(repo) = self.repos.borrow_mut().iter_mut().find(|repo| repo.root == root) {
                        repo.branches = branches;
                    }
                }
                Event::Head(file, content) => {
                    let key = crate::git::key(&file);
                    self.head_requests.borrow_mut().remove(&key);
                    self.heads.borrow_mut().insert(key, content);
                    if self.active_path().as_deref() == Some(file.as_path()) {
                        self.update_markers();
                    }
                }
                Event::Finished(root, result) => {
                    if let Err(error) = result {
                        message_box(self.hwnd, &error, MB_ICONERROR);
                    }
                    self.worker.refresh(&root);
                    if self.panel.visible.get() {
                        self.worker.log(&root);
                    }
                }
            }
        }
        self.invalidate();
    }

    fn git_color(&self, param: LPARAM) -> Option<u32> {
        let path = self.explorer.path_of_param(param)?;
        let palette = &self.palette;
        Some(match self.colors.borrow().of(&path)? {
            FileStatus::Added => palette.git_added,
            FileStatus::Modified | FileStatus::Deleted => palette.git_modified,
            FileStatus::Untracked => palette.git_untracked,
            FileStatus::Ignored => palette.git_ignored,
            FileStatus::Conflict => palette.danger,
        })
    }

    fn push(&self) {
        if let Some(root) = self.active_repo_root() {
            self.worker.run(&root, |git, root| git.push(root));
        }
    }

    /// Menu VCS (clic sul branch nella status bar o Alt+`): pull, push, fetch, nuovo branch, checkout, merge.
    fn vcs_menu(&self) {
        let Some(root) = self.active_repo_root() else { return };
        let branches: Vec<(String, bool, bool)> = self
            .repos
            .borrow()
            .iter()
            .find(|repo| repo.root == root)
            .map(|repo| repo.branches.iter().map(|branch| (branch.name.clone(), branch.remote, branch.current)).collect())
            .unwrap_or_default();
        let anchor = self.vcs_rect.get();
        // SAFETY: menu creato, mostrato e distrutto qui (DestroyMenu distrugge anche i sottomenu); le stringhe vivono per tutta la chiamata.
        let command = unsafe {
            let menu = CreatePopupMenu();
            let labels: Vec<Vec<u16>> = branches.iter().map(|(name, _, _)| wide(name)).collect();
            let fixed: Vec<(usize, Vec<u16>)> = [(VCS_PULL, "Aggiorna (pull)"), (VCS_PUSH, "Push\tCtrl+Shift+K"), (VCS_FETCH, "Fetch"), (VCS_NEW_BRANCH, "Nuovo branch…")]
                .iter()
                .map(|(id, label)| (*id, wide(label)))
                .collect();
            for (index, (id, label)) in fixed.iter().enumerate() {
                if index == 3 {
                    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                }
                AppendMenuW(menu, MF_STRING, *id, label.as_ptr());
            }
            let checkout = CreatePopupMenu();
            let merge = CreatePopupMenu();
            for (index, ((_, remote, current), label)) in branches.iter().zip(&labels).enumerate() {
                let flags = if *current { MF_STRING | MF_CHECKED | MF_GRAYED } else { MF_STRING };
                if *remote && index > 0 && !branches[index - 1].1 {
                    AppendMenuW(checkout, MF_SEPARATOR, 0, std::ptr::null());
                }
                AppendMenuW(checkout, flags, VCS_CHECKOUT + index, label.as_ptr());
                if !current {
                    AppendMenuW(merge, MF_STRING, VCS_MERGE + index, label.as_ptr());
                }
            }
            let (checkout_label, merge_label) = (wide("Checkout"), wide("Merge nel branch corrente"));
            AppendMenuW(menu, MF_POPUP, checkout as usize, checkout_label.as_ptr());
            AppendMenuW(menu, MF_POPUP, merge as usize, merge_label.as_ptr());
            let mut point = POINT { x: anchor.left, y: anchor.top };
            ClientToScreen(self.hwnd, &mut point);
            let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_BOTTOMALIGN | TPM_LEFTALIGN, point.x, point.y, 0, self.hwnd, std::ptr::null());
            DestroyMenu(menu);
            command as usize
        };
        let branch = |offset: usize| branches.get(command - offset).cloned();
        match command {
            VCS_PULL => {
                let rebase = self.settings.pull_rebase();
                self.worker.run(&root, move |git, root| git.pull(root, rebase));
            }
            VCS_PUSH => self.push(),
            VCS_FETCH => self.worker.run(&root, |git, root| git.fetch(root)),
            VCS_NEW_BRANCH => {
                if let Some(name) = prompt(self.hwnd, &self.palette, self.font.get(), "Nuovo branch", "Nome del nuovo branch (parte da HEAD e diventa quello corrente):", "") {
                    self.worker.run(&root, move |git, root| git.create_branch(root, &name, true, None));
                }
            }
            id if (VCS_CHECKOUT..VCS_MERGE).contains(&id) => {
                if let Some((name, remote, _)) = branch(VCS_CHECKOUT) {
                    self.worker.run(&root, move |git, root| git.switch(root, &name, remote));
                }
            }
            id if id >= VCS_MERGE => {
                if let Some((name, _, _)) = branch(VCS_MERGE) {
                    self.worker.run(&root, move |git, root| git.merge(root, &name));
                }
            }
            _ => {}
        }
    }

    /// Mostra il pannello sulla tab indicata, per il repository del file attivo.
    fn show_panel(&self, tab: Tab) {
        self.panel.visible.set(true);
        self.panel.tab.set(tab);
        *self.panel.root.borrow_mut() = self.active_repo_root();
        self.sync_panel();
        self.layout();
        let focus = match tab {
            Tab::Console => self.panel.console.hwnd,
            Tab::Git => self.panel.log.hwnd,
            Tab::Commit => self.panel.message,
        };
        // SAFETY: controllo valido.
        unsafe { SetFocus(focus) };
    }

    /// Alt+9 e simili: apre la tab, o chiude il pannello se è già quella mostrata.
    fn toggle_panel(&self, tab: Tab) {
        if self.panel.visible.get() && self.panel.tab.get() == tab {
            self.panel.visible.set(false);
            self.layout();
            self.focus_editor();
        } else {
            self.show_panel(tab);
        }
    }

    fn sync_panel(&self) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        if let Some(status) = self.repos.borrow().iter().find(|repo| repo.root == root).and_then(|repo| repo.status.as_ref()) {
            self.panel.set_changes(&root, status);
        }
        if self.panel.tab.get() == Tab::Git {
            self.worker.log(&root);
        }
    }

    fn commit(&self, push: bool) {
        let Some(root) = self.panel.root.borrow().clone().or_else(|| self.active_repo_root()) else { return };
        let message = window_text(self.panel.message).trim().to_string();
        if message.is_empty() {
            self.show_panel(Tab::Commit);
            message_box(self.hwnd, "Scrivi il messaggio di commit.", MB_ICONINFORMATION);
            return;
        }
        let (files, unstage): (Vec<String>, Vec<String>) = {
            let changes = self.panel.changes.borrow();
            (
                changes.iter().filter(|file| file.checked).map(|file| file.path.clone()).collect(),
                changes.iter().filter(|file| !file.checked && file.staged).map(|file| file.path.clone()).collect(),
            )
        };
        if files.is_empty() {
            message_box(self.hwnd, "Nessun file selezionato per il commit.", MB_ICONINFORMATION);
            return;
        }
        self.worker.commit(&root, files, unstage, message, push);
    }

    fn pull(&self) {
        if let Some(root) = self.active_repo_root() {
            let rebase = self.settings.pull_rebase();
            self.worker.run(&root, move |git, root| git.pull(root, rebase));
        }
    }

    /// Menu contestuale di un commit nel log.
    fn log_menu(&self, index: usize) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let Some(hash) = self.panel.commits.borrow().get(index).map(|commit| commit.hash.clone()) else { return };
        let entries = [
            (LOG_CHECKOUT, "Checkout"),
            (LOG_BRANCH, "Nuovo branch da qui…"),
            (LOG_TAG, "Crea tag…"),
            (0, ""),
            (LOG_CHERRY_PICK, "Cherry-pick"),
            (LOG_REVERT, "Revert"),
            (0, ""),
            (LOG_RESET_SOFT, "Reset soft"),
            (LOG_RESET_MIXED, "Reset mixed"),
            (LOG_RESET_HARD, "Reset hard"),
            (0, ""),
            (LOG_COPY_HASH, "Copia hash"),
        ];
        let short = hash.get(..7).unwrap_or(&hash).to_string();
        match popup_menu(self.hwnd, &entries) {
            LOG_CHECKOUT => self.worker.run(&root, move |git, root| git.checkout_commit(root, &hash)),
            LOG_BRANCH => {
                if let Some(name) = prompt(self.hwnd, &self.palette, self.font.get(), "Nuovo branch", &format!("Nome del branch che parte da {short}:"), "") {
                    self.worker.run(&root, move |git, root| git.create_branch(root, &name, true, Some(&hash)));
                }
            }
            LOG_TAG => {
                if let Some(name) = prompt(self.hwnd, &self.palette, self.font.get(), "Crea tag", &format!("Nome del tag su {short}:"), "") {
                    self.worker.run(&root, move |git, root| git.create_tag(root, &name, &hash));
                }
            }
            LOG_CHERRY_PICK => self.worker.run(&root, move |git, root| git.cherry_pick(root, &hash)),
            LOG_REVERT => self.worker.run(&root, move |git, root| git.revert(root, &hash)),
            LOG_RESET_SOFT => self.worker.run(&root, move |git, root| git.reset(root, &hash, ResetMode::Soft)),
            LOG_RESET_MIXED => self.worker.run(&root, move |git, root| git.reset(root, &hash, ResetMode::Mixed)),
            LOG_RESET_HARD => {
                let question = format!("Reset hard a {short}? Le modifiche non committate andranno perse.");
                if message_box(self.hwnd, &question, MB_YESNO | MB_ICONWARNING) == IDYES {
                    self.worker.run(&root, move |git, root| git.reset(root, &hash, ResetMode::Hard));
                }
            }
            LOG_COPY_HASH => copy_to_clipboard(self.hwnd, &hash),
            _ => {}
        }
    }

    fn on_list_event(&self, list: HWND, event: ListEvent, index: usize) {
        if list == self.panel.log.hwnd {
            if event == ListEvent::Menu {
                self.log_menu(index);
            }
            return;
        }
        if list != self.panel.files.hwnd {
            return;
        }
        match event {
            ListEvent::Toggle => self.panel.toggle(index),
            ListEvent::Select if self.panel.on_checkbox(|value| self.scale(value)) => self.panel.toggle(index),
            ListEvent::Activate => {
                let path = self.panel.changes.borrow().get(index).map(|file| PathBuf::from(&file.path));
                if let Some(path) = path.filter(|path| path.is_file()) {
                    self.open(&path);
                }
            }
            _ => {}
        }
        // SAFETY: area del pannello nella finestra principale (contatore dei file selezionati).
        unsafe { InvalidateRect(self.hwnd, &self.panel_rect(), 0) };
    }

    fn panel_height(&self) -> i32 {
        if !self.panel.visible.get() {
            return 0;
        }
        let available = self.status_rect().top - self.scale(TAB_HEIGHT) - self.scale(EDITOR_MIN);
        self.scale(self.panel.height.get()).clamp(self.scale(panel::HEADER + 60), available.max(self.scale(panel::HEADER + 60)))
    }

    fn panel_rect(&self) -> RECT {
        let status = self.status_rect();
        RECT { top: status.top - self.panel_height(), bottom: status.top, ..status }
    }

    fn over_panel_splitter(&self, y: i32) -> bool {
        self.panel.visible.get() && (y - self.panel_rect().top).abs() <= self.scale(3)
    }

    fn tab_at(&self, x: i32, y: i32) -> Option<(usize, bool)> {
        let tabs = self.tabs.borrow();
        let index = tabs.iter().position(|(tab, _)| contains(tab, x, y))?;
        Some((index, contains(&tabs[index].1, x, y)))
    }

    fn on_click(&self, x: i32, y: i32) {
        if self.over_panel_splitter(y) {
            self.dragging_panel.set(true);
            // SAFETY: cattura del mouse sulla finestra principale fino al rilascio.
            unsafe { SetCapture(self.hwnd) };
            return;
        }
        if self.panel.visible.get() && contains(&self.panel_rect(), x, y) {
            if let Some(tab) = self.panel.tab_at(x, y) {
                return self.show_panel(tab);
            }
            match self.panel.button_at(x, y) {
                Some(panel::Button::Commit) => self.commit(false),
                Some(panel::Button::CommitAndPush) => self.commit(true),
                None => {}
            }
            return;
        }
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
        if contains(&self.vcs_rect.get(), x, y) {
            return self.vcs_menu();
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

    fn on_mouse_move(&self, x: i32, y: i32) {
        if self.dragging_panel.get() {
            let height = self.status_rect().top - y;
            self.panel.height.set(height * 96 / self.dpi().max(1));
            return self.layout();
        }
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
        let clickable = contains(&self.eol_rect.get(), point.x, point.y) || contains(&self.vcs_rect.get(), point.x, point.y) || (self.explorer_visible.get() && contains(&self.header_rect(), point.x, point.y));
        let over_panel = self.over_panel_splitter(point.y) || self.dragging_panel.get();
        let cursor = match (over_splitter || self.dragging.get(), clickable) {
            _ if over_panel => IDC_SIZENS,
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
            let (x, y) = point(lparam);
            app.on_mouse_move(x, y);
            0
        }
        WM_LBUTTONUP => {
            if app.dragging.replace(false) | app.dragging_panel.replace(false) {
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
            if header.hwndFrom == app.explorer.hwnd && header.code == NM_CUSTOMDRAW {
                let draw = &mut *(lparam as *mut NMTVCUSTOMDRAW);
                return match draw.nmcd.dwDrawStage {
                    CDDS_PREPAINT => CDRF_NOTIFYITEMDRAW as LRESULT,
                    CDDS_ITEMPREPAINT => {
                        if let Some(color) = app.git_color(draw.nmcd.lItemlParam) {
                            draw.clrText = color;
                        }
                        CDRF_DODEFAULT as LRESULT
                    }
                    _ => CDRF_DODEFAULT as LRESULT,
                };
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
        WM_GIT_EVENT => {
            app.on_git_events();
            0
        }
        WM_COMMAND_LOGGED => {
            app.panel.append_console(&app.worker.take_log());
            0
        }
        WM_LIST_PAINT => {
            let row = &*(lparam as *const RowPaint);
            if row.list == app.panel.log.hwnd {
                app.panel.paint_log_row(row, &app.palette, app.font.get(), |value| app.scale(value));
            } else {
                app.panel.paint_file_row(row, &app.palette, app.font.get(), |value| app.scale(value));
            }
            0
        }
        WM_LIST_EVENT => {
            let (event, index) = decode_event(lparam);
            app.on_list_event(wparam as HWND, event, index);
            0
        }
        WM_TIMER if wparam == MARKER_TIMER => {
            KillTimer(hwnd, MARKER_TIMER);
            app.update_markers();
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

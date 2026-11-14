//! Finestra principale: explorer a sinistra, tab, barra di ricerca ed editor Scintilla a destra, status bar in basso.
//! Tab, intestazioni e status bar sono disegnate a mano; c'è un documento Scintilla per tab.
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Controls::{SetWindowTheme, WM_MOUSELEAVE, TTTOOLINFOW, TTF_SUBCLASS, TTM_ADDTOOLW, TTM_NEWTOOLRECTW, TTM_POP, TTM_UPDATETIPTEXTW, TTS_ALWAYSTIP, TTS_NOPREFIX, CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDRF_DODEFAULT, CDRF_NOTIFYITEMDRAW, NMHDR, NMTVCUSTOMDRAW, NM_CUSTOMDRAW};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use steroitor_core::git::history::ResetMode;
use steroitor_core::git::history::LogFilter;
use steroitor_core::git::parse::{BlameLine, FileStatus, Operation};
use steroitor_core::text::{decode, encode};

use crate::compare::Compare;
use crate::dialogs::{copy_to_clipboard, message_box, open_files, pick_folder, save_file, wide};
use crate::explorer::{popup_menu, Action, Explorer};
use crate::find::{self, window_text, FindBar};
use crate::hunks;
use crate::git::{line_changes, repo_name, repo_of, CommitRequest, Event, FileColors, Repo, Worker, WM_COMMAND_LOGGED, WM_GIT_EVENT};
use crate::lang::{self, Language};
use crate::minimap::{self, Minimap, WM_MINIMAP_LINE};
use crate::list::{decode_event, ListEvent, RowPaint, WM_LIST_EVENT, WM_LIST_PAINT};
use crate::paint::{fill, frame, text, text_width};
use crate::panel::{self, BranchEntry, BranchRow, ConsoleTarget, DetailRow, Panel, RepoTab, Tab};
use crate::plugins::{self, Host};
use crate::prompt::{prompt, prompt_selected, prompt_text};
use crate::quick_open;
use crate::sci::*;
use crate::session::{ProjectSession, Session, Settings, UnsavedTab};
use crate::settings_page;
use crate::terminal::{Terminal, WM_TERMINAL_EXIT, WM_TERMINAL_OUTPUT};
use crate::theme::{system_palette, Palette};


const CLASS_NAME: &str = "SteroitorMain";
const TAB_HEIGHT: i32 = 34;
/// Barra in alto (Nuovo, Apri, …) e intestazione dell'explorer, come nella versione Tauri.
const TOP_BAR: i32 = 32;
const EXPLORER_HEADER: i32 = 30;
const BADGE_FONT_PX: i32 = 10;
const STATUS_HEIGHT: i32 = 24;
const SPLITTER: i32 = 4;
const TAB_MAX_WIDTH: i32 = 220;
const CLOSE_SIZE: i32 = 16;
const UI_FONT_PX: i32 = 13;
const TREE_ITEM_HEIGHT: i32 = 22;
const EXPLORER_WIDTH: i32 = 240;
const EXPLORER_MIN: i32 = 120;
const EDITOR_MIN: i32 = 200;
const PROJECT_BAR: i32 = 36;
const PROJECT_BUTTON: i32 = 28;
/// Striscia delle modifiche accanto all'editor: un segno per ogni riga cambiata, in proporzione al file.
const RULER: i32 = 10;
const SCI_GETLINECOUNT: u32 = 2154;
const SCI_LINESONSCREEN: u32 = 2370;
const SCI_VISIBLEFROMDOCLINE: u32 = 2220;
/// Il watcher (altro thread) avvisa la finestra con questo messaggio; le cartelle cambiate aspettano in `changes`.
const WM_FILES_CHANGED: u32 = WM_APP + 1;
const VCS_COMMIT: usize = 8;
const VCS_FETCH_ALL: usize = 9;
const VCS_HISTORY: usize = 10;
const VCS_HISTORY_SELECTION: usize = 11;
const VCS_METHOD_MERGE: usize = 12;
const VCS_METHOD_REBASE: usize = 13;
const VCS_BRANCH_FROM: usize = 9000;
const EDIT_DECLARATION: usize = 1;
const EDIT_CUT: usize = 2;
const EDIT_COPY: usize = 3;
const EDIT_PASTE: usize = 4;
const EDIT_SELECT_ALL: usize = 5;
const EDIT_BLAME: usize = 6;
const EDIT_HISTORY: usize = 7;
const EDIT_HISTORY_SELECTION: usize = 8;
const SCI_POSITIONFROMPOINT: u32 = 2022;
const SCI_CUT: u32 = 2177;
const SCI_COPY: u32 = 2178;
const SCI_PASTE: u32 = 2179;
const SCI_SELECTALL: u32 = 2013;
const SCI_USEPOPUP: u32 = 2371;
const SCI_MARKERNEXT: u32 = 2047;
const SCI_GOTOLINE: u32 = 2024;
const SCN_MARGINRIGHTCLICK: u32 = 2031;
const TREE_INDENT: i32 = 14;
const HEADER_FONT_PX: i32 = 11;const MARKER_TIMER: usize = 1;
const MARKER_DELAY_MS: u32 = 300;
const GIT_MARGIN: usize = 1;
const BLAME_MARGIN: usize = 2;
const LOG_FILTER_TIMER: usize = 4;
const LOG_FILTER_DELAY_MS: u32 = 400;
const GUTTER_ROLLBACK: usize = 1;
const GUTTER_STAGE: usize = 2;
const GUTTER_DIFF: usize = 3;
const GUTTER_BLAME: usize = 4;
const BRANCH_LOG: usize = 1;
const BRANCH_CHECKOUT: usize = 2;
const BRANCH_MERGE: usize = 3;
const BRANCH_REBASE: usize = 4;
const BRANCH_NEW: usize = 5;
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
const LOG_SHOW_CHANGES: usize = 10;
const LOG_REWORD: usize = 11;
const FILE_DIFF: usize = 1;
const FILE_OPEN: usize = 2;
const LIST_ROW_HEIGHT: i32 = 24;
const VCS_PULL: usize = 1;
const VCS_PUSH: usize = 2;
const VCS_FETCH: usize = 3;
const VCS_NEW_BRANCH: usize = 4;
const VCS_CHECKOUT: usize = 1000;
const VCS_MERGE: usize = 3000;
const VCS_REBASE: usize = 5000;
const VCS_STASH_POP: usize = 7000;
const VCS_STASH_DROP: usize = 8000;
const VCS_CONTINUE: usize = 5;
const VCS_ABORT: usize = 6;
const VCS_STASH_PUSH: usize = 7;

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
    /// Annotazioni (blame) mostrate nel margine, una per riga; vuoto se spente.
    blame: Vec<BlameLine>,
}

impl Doc {
    fn name(&self) -> String {
        match &self.path {
            Some(path) => path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned()),
            None => format!("Senza titolo {}", self.untitled),
        }
    }
}

/// Annotazione di una riga: hash breve, data, autore.
fn blame_label(entry: &BlameLine) -> String {
    if entry.hash.bytes().all(|byte| byte == b'0') {
        return format!(" {}", entry.author);
    }
    let date = panel::local_time(entry.timestamp);
    format!(" {} {} {}", entry.hash.get(..7).unwrap_or(&entry.hash), date.get(..10).unwrap_or(&date), entry.author)
}

/// Pulsanti dell'intestazione dell'explorer e del suo stato vuoto.
#[derive(Clone, Copy, PartialEq)]
enum HeaderButton {
    NewFile,
    Refresh,
    Collapse,
    OpenFolder,
    Clone,
}

/// Pulsanti della barra in alto, nell'ordine della versione Tauri.
#[derive(Clone, Copy, PartialEq)]
enum TopButton {
    New,
    Open,
    OpenFolder,
    Clone,
    NewProject,
    Settings,
    Explorer,
    Panel,
}

/// Icona da un PNG (CreateIconFromResourceEx accetta il formato PNG delle icone).
fn png_icon(png: &[u8], size: i32) -> HICON {
    // SAFETY: dati del PNG validi per la chiamata; l'icona creata vive quanto l'app.
    unsafe { CreateIconFromResourceEx(png.as_ptr(), png.len() as u32, 1, 0x0003_0000, size, size, LR_DEFAULTCOLOR) }
}

fn operation_name(operation: &Operation) -> &'static str {
    match operation {
        Operation::Merge => "merge",
        Operation::Rebase => "rebase",
        Operation::CherryPick => "cherry-pick",
        Operation::Revert => "revert",
    }
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

pub struct App {
    hwnd: HWND,
    sci: Sci,
    /// Vista nascosta per leggere il testo dei documenti non mostrati (le tab modificate da salvare nella sessione).
    scratch: Sci,
    /// Tab che si sta trascinando per riordinarla.
    tab_drag: Cell<Option<usize>>,
    minimap: Minimap,
    minimap_on: Cell<bool>,
    minimap_rect: Cell<RECT>,
    /// Righe cambiate rispetto a HEAD e colore, per la striscia delle modifiche.
    ruler: RefCell<Vec<(usize, u32)>>,
    explorer: Explorer,
    panel: Panel,
    dragging_panel: Cell<bool>,
    find: FindBar,
    palette: Palette,
    settings: RefCell<Settings>,
    worker: Worker,
    compare: &'static Compare,
    plugins: Host,
    repos: RefCell<Vec<Repo>>,
    colors: RefCell<FileColors>,
    /// Contenuto in HEAD dei file aperti (None = non tracciato), per i marker nel gutter.
    heads: RefCell<HashMap<String, Option<String>>>,
    head_requests: RefCell<std::collections::HashSet<String>>,
    docs: RefCell<Vec<Doc>>,
    active: Cell<usize>,
    next_untitled: Cell<u32>,
    font: Cell<HFONT>,
    edit_brush: HBRUSH,
    tabs: RefCell<Vec<(RECT, RECT)>>,
    top_buttons: RefCell<Vec<(RECT, TopButton)>>,
    explorer_icons: Cell<windows_sys::Win32::UI::Controls::HIMAGELIST>,
    header_font: Cell<HFONT>,
    header_buttons: RefCell<Vec<(RECT, HeaderButton)>>,
    /// Ultima posizione del mouse nella finestra, per gli effetti al passaggio; None quando esce.
    pointer: Cell<Option<(i32, i32)>>,
    tracking: Cell<bool>,
    /// Elementi della status bar cliccabili: un branch per repository, l'interruttore dell'a capo.
    vcs_rects: RefCell<Vec<(RECT, PathBuf)>>,
    wrap_rect: Cell<RECT>,
    app_icon: HICON,
    badge_font: Cell<HFONT>,
    eol_rect: Cell<RECT>,
    explorer_width: Cell<i32>,
    explorer_visible: Cell<bool>,
    dragging: Cell<bool>,
    wrap: Cell<bool>,
    watcher: RefCell<Option<Debouncer<RecommendedWatcher>>>,
    /// Dal watcher: cartelle cambiate e repository con metadati Git cambiati.
    changes: RefCell<Changes>,
    /// I progetti della finestra, nell'ordine della barra: quello attivo vive nei campi qui sopra e ha il posto vuoto.
    projects: RefCell<Vec<Option<Parked>>>,
    current: Cell<usize>,
    /// Pulsanti della barra dei progetti; l'ultimo è il `+`.
    project_rects: RefCell<Vec<RECT>>,
    tooltip: HWND,
    hovered: Cell<Option<usize>>,
}

type Changes = Arc<Mutex<(Vec<String>, Vec<PathBuf>)>>;

/// Un progetto non attivo: gli stessi campi che `App` ha per quello attivo, scambiati al cambio di progetto.
struct Parked {
    explorer: Explorer,
    docs: Vec<Doc>,
    active: usize,
    repos: Vec<Repo>,
    colors: FileColors,
    heads: HashMap<String, Option<String>>,
    watcher: Option<Debouncer<RecommendedWatcher>>,
    /// Le modifiche viste dal watcher mentre il progetto non è attivo aspettano qui.
    changes: Changes,
    panel: panel::ProjectState,
    /// Ripristinato e mai aperto: cosa caricare alla prima attivazione.
    pending: Option<ProjectSession>,
}

impl Parked {
    fn folder(&self) -> Option<PathBuf> {
        self.pending.as_ref().map_or_else(|| self.explorer.root(), |pending| pending.folder.clone())
    }

    fn session(&self, text_of: impl Fn(isize) -> String) -> ProjectSession {
        if let Some(pending) = &self.pending {
            return pending.clone();
        }
        session_of(self.explorer.root(), &self.docs, self.active, text_of)
    }
}

/// Cartella, file con un percorso, tab attiva (contata tra quei file) e testo delle tab modificate o senza titolo.
fn session_of(folder: Option<PathBuf>, docs: &[Doc], active: usize, text_of: impl Fn(isize) -> String) -> ProjectSession {
    let saved: Vec<(usize, PathBuf)> = docs.iter().enumerate().filter_map(|(index, doc)| doc.path.clone().map(|path| (index, path))).collect();
    let active = saved.iter().position(|(index, _)| *index == active).unwrap_or(0);
    let unsaved = docs
        .iter()
        .filter(|doc| doc.dirty)
        .map(|doc| UnsavedTab { path: doc.path.clone(), content: text_of(doc.pointer), encoding: doc.encoding.clone(), bom: doc.bom })
        .collect();
    ProjectSession { folder, files: saved.into_iter().map(|(_, path)| path).collect(), active, unsaved }
}

/// Invia file e cartelle alla finestra già aperta (se c'è) e la porta in primo piano; true se l'ha trovata.
pub fn forward_to_running(paths: &[PathBuf]) -> bool {
    use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
    let class = wide(CLASS_NAME);
    // SAFETY: la struttura e il testo vivono per tutta la SendMessageW, che è sincrona.
    unsafe {
        let existing = FindWindowW(class.as_ptr(), std::ptr::null());
        if existing.is_null() {
            return false;
        }
        let joined = paths.iter().map(|path| std::path::absolute(path).unwrap_or_else(|_| path.clone()).to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n");
        let text: Vec<u16> = joined.encode_utf16().collect();
        let data = COPYDATASTRUCT { dwData: COPYDATA_OPEN, cbData: (text.len() * 2) as u32, lpData: text.as_ptr() as *mut _ };
        SendMessageW(existing, WM_COPYDATA, 0, &data as *const COPYDATASTRUCT as isize);
        if IsIconic(existing) != 0 {
            ShowWindow(existing, SW_RESTORE);
        }
        SetForegroundWindow(existing);
        true
    }
}

const COPYDATA_OPEN: usize = 0x5354_4f50;

fn folder_name(folder: Option<&Path>) -> String {
    folder.map_or_else(|| "Senza cartella".into(), |folder| folder.file_name().map_or_else(|| folder.display().to_string(), |name| name.to_string_lossy().into_owned()))
}

/// Iniziale mostrata nella barra dei progetti.
fn folder_initial(folder: Option<&Path>) -> String {
    folder.and_then(|folder| folder.file_name()).and_then(|name| name.to_string_lossy().chars().find(|ch| ch.is_alphanumeric())).map_or_else(|| "?".into(), |ch| ch.to_uppercase().collect())
}

/// Tooltip con il nome del progetto sotto il mouse, sulla barra dei progetti.
fn create_tooltip(owner: HWND) -> HWND {
    let class = wide("tooltips_class32");
    // SAFETY: finestra di tooltip posseduta dalla finestra principale; la struttura vive per la chiamata.
    unsafe {
        let tooltip = CreateWindowExW(WS_EX_TOPMOST, class.as_ptr(), std::ptr::null(), WS_POPUP | TTS_NOPREFIX | TTS_ALWAYSTIP, 0, 0, 0, 0, owner, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null());
        let mut empty = [0u16];
        let mut tool: TTTOOLINFOW = std::mem::zeroed();
        tool.cbSize = std::mem::size_of::<TTTOOLINFOW>() as u32;
        tool.uFlags = TTF_SUBCLASS;
        tool.hwnd = owner;
        tool.lpszText = empty.as_mut_ptr();
        SendMessageW(tooltip, TTM_ADDTOOLW, 0, &tool as *const _ as isize);
        tooltip
    }
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
                style: CS_DBLCLKS,
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

        let settings = Settings::load();
        let worker = Worker::new(hwnd);
        let app: &'static App = Box::leak(Box::new(App {
            hwnd,
            sci: Sci::new(editor),
            // SAFETY: controllo figlio nascosto della finestra principale.
            scratch: Sci::new(unsafe { CreateWindowExW(0, wide("Scintilla").as_ptr(), std::ptr::null(), WS_CHILD, 0, 0, 0, 0, hwnd, std::ptr::null_mut(), instance, std::ptr::null()) }),
            tab_drag: Cell::new(None),
            minimap: Minimap::create(hwnd, instance, editor, &palette),
            minimap_on: Cell::new(session.minimap),
            minimap_rect: Cell::new(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
            ruler: RefCell::new(Vec::new()),
            explorer: Explorer::create(hwnd, instance, &palette),
            panel: Panel::create(hwnd, instance, &palette),
            dragging_panel: Cell::new(false),
            find: FindBar::create(hwnd, instance),
            palette,
            compare: Compare::create(hwnd, instance, &palette, settings.editor.font_size_fraction(), worker.clone()),
            settings: RefCell::new(settings),
            worker,
            plugins: Host::new(hwnd, crate::session::data_dir(), palette),
            repos: RefCell::new(Vec::new()),
            colors: RefCell::new(FileColors::default()),
            heads: RefCell::new(HashMap::new()),
            head_requests: RefCell::default(),
            docs: RefCell::new(Vec::new()),
            active: Cell::new(0),
            next_untitled: Cell::new(1),
            font: Cell::new(std::ptr::null_mut()),
            // SAFETY: pennello creato una volta e usato per tutta la vita dell'app.
            edit_brush: unsafe { CreateSolidBrush(palette.editor) },
            tabs: RefCell::new(Vec::new()),
            top_buttons: RefCell::new(Vec::new()),
            explorer_icons: Cell::new(0),
            header_font: Cell::new(std::ptr::null_mut()),
            header_buttons: RefCell::new(Vec::new()),
            pointer: Cell::new(None),
            tracking: Cell::new(false),
            vcs_rects: RefCell::new(Vec::new()),
            wrap_rect: Cell::new(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
            app_icon: png_icon(include_bytes!("../../src-tauri/icons/32x32.png"), 32),
            badge_font: Cell::new(std::ptr::null_mut()),
            eol_rect: Cell::new(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
            explorer_width: Cell::new(session.explorer_width.unwrap_or(EXPLORER_WIDTH)),
            explorer_visible: Cell::new(session.explorer_visible.unwrap_or(true)),
            dragging: Cell::new(false),
            wrap: Cell::new(session.wrap),
            watcher: RefCell::new(None),
            changes: RefCell::default(),
            projects: RefCell::new(Vec::new()),
            current: Cell::new(0),
            project_rects: RefCell::new(Vec::new()),
            tooltip: create_tooltip(hwnd),
            hovered: Cell::new(None),
        }));
        APP.with(|cell| cell.set(app).ok());
        // SAFETY: icone della finestra create qui e tenute per tutta la vita dell'app.
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, app.app_icon as isize);
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, png_icon(include_bytes!("../../src-tauri/icons/128x128.png"), 128) as isize);
        }
        app.panel.visible.set(session.panel_visible.unwrap_or(true));
        app.panel.height.set(session.panel_height.unwrap_or(200));
        app.panel.tab.set(match session.panel_tab.as_deref() {
            Some("git") => Tab::Git,
            Some("commit") => Tab::Commit,
            _ => Tab::Console,
        });
        app.panel.attach();
        app.configure_editor();
        app.update_font();

        // Il progetto attivo si carica subito, gli altri quando li si apre.
        let saved = session.project_list();
        let active = session.active_project.min(saved.len() - 1);
        for (index, project) in saved.iter().enumerate() {
            let parked = (index != active).then(|| app.park(Some(project.clone())));
            app.projects.borrow_mut().push(parked);
        }
        app.current.set(active);
        app.load_project(saved[active].clone());

        // Da riga di comando: una cartella apre (o porta in primo piano) il suo progetto, i file vanno in quello attivo.
        let (folders, files): (Vec<PathBuf>, Vec<PathBuf>) = arguments.into_iter().partition(|path| path.is_dir());
        if let Some(folder) = folders.into_iter().next() {
            app.open_project(folder);
        }
        for file in files.iter().filter(|file| file.is_file()) {
            app.open(file);
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
            let icons = crate::icons::image_list(&self.palette, self.scale(16));
            self.explorer.set_font(font, self.scale(TREE_ITEM_HEIGHT), self.scale(TREE_INDENT));
            self.explorer.set_image_list(icons);
            for parked in self.projects.borrow().iter().flatten() {
                parked.explorer.set_font(font, self.scale(TREE_ITEM_HEIGHT), self.scale(TREE_INDENT));
                parked.explorer.set_image_list(icons);
            }
            let old_icons = self.explorer_icons.replace(icons);
            if old_icons != 0 {
                windows_sys::Win32::UI::Controls::ImageList_Destroy(old_icons);
            }
            let header = CreateFontW(-self.scale(HEADER_FONT_PX), 0, 0, 0, 600, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, face.as_ptr());
            let old_header = self.header_font.replace(header);
            if !old_header.is_null() {
                DeleteObject(old_header);
            }
            self.plugins.set_font(font);
            for control in [self.panel.message, self.panel.filter_text, self.panel.filter_author, self.panel.filter_path] {
                SendMessageW(control, WM_SETFONT, font as usize, 1);
            }
            for list in [&self.panel.branches, &self.panel.log, &self.panel.details, &self.panel.files] {
                list.set_row_height(self.scale(LIST_ROW_HEIGHT));
            }
            let badge = CreateFontW(-self.scale(BADGE_FONT_PX), 0, 0, 0, 600, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, face.as_ptr());
            let old_badge = self.badge_font.replace(badge);
            if !old_badge.is_null() {
                DeleteObject(old_badge);
            }
            let old = self.font.replace(font);
            if !old.is_null() {
                DeleteObject(old);
            }
        }
    }

    fn configure_editor(&self) {
        let sci = &self.sci;
        sci.call(SCI_SETMULTIPLESELECTION, 1, 0);
        sci.call(SCI_USEPOPUP, 0, 0);
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
        sci.call(SCI_SETMARGINWIDTHN, GIT_MARGIN, self.scale(6) as isize);
        sci.call(SCI_SETMARGINSENSITIVEN, GIT_MARGIN, 1);
        sci.call(SCI_SETMARGINTYPEN, BLAME_MARGIN, SC_MARGIN_TEXT);
        sci.call(SCI_SETMARGINWIDTHN, BLAME_MARGIN, 0);
        sci.call(SCI_SETMARGINSENSITIVEN, BLAME_MARGIN, 1);
        sci.call(SCI_SETMARGINMASKN, BLAME_MARGIN, 0);
        for (marker, color) in [(MARKER_ADDED, self.palette.git_added), (MARKER_MODIFIED, self.palette.git_modified), (MARKER_DELETED, self.palette.danger)] {
            sci.call(SCI_MARKERDEFINE, marker, SC_MARK_LEFTRECT);
            sci.call(SCI_MARKERSETBACK, marker, color as isize);
            self.minimap.define_marker(marker, SC_MARK_LEFTRECT, color);
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
        sci.call(SCI_SETMARGINBACKN, BLAME_MARGIN, palette.panel as isize);
        sci.call(SCI_INDICSETSTYLE, find::INDICATOR, INDIC_ROUNDBOX);
        sci.call(SCI_INDICSETFORE, find::INDICATOR, palette.accent as isize);
        sci.call(SCI_INDICSETALPHA, find::INDICATOR, 60);
        sci.call(SCI_INDICSETOUTLINEALPHA, find::INDICATOR, 160);
        sci.call(SCI_INDICSETUNDER, find::INDICATOR, 1);
        Host::configure(sci, palette);
    }

    /// Le funzioni dei plugin vogliono editor, impostazioni e percorso della tab attiva.
    fn with_plugins(&self, action: impl FnOnce(&Host, &Sci, &Settings, Option<&Path>)) {
        let path = self.active_path();
        action(&self.plugins, &self.sci, &self.settings.borrow(), path.as_deref());
    }

    /// Stili e interlinea sono della vista: si riapplicano a ogni cambio di tab.
    fn apply_styles(&self, language: &Language) {
        let style = self.settings.borrow().editor;
        lang::apply_styles(&self.sci, language, &self.palette, style.font_size_fraction());
        self.minimap.show_document(self.sci.call(SCI_GETDOCPOINTER, 0, 0), language, &self.palette, self.wrap.get());
        let (ascent, descent) = style.line_spacing(self.dpi());
        self.sci.call(SCI_SETEXTRAASCENT, ascent as usize, 0);
        self.sci.call(SCI_SETEXTRADESCENT, descent as usize, 0);
    }

    /// Tab e indentazione sono del documento attivo.
    fn apply_indentation(&self) {
        let style = self.settings.borrow().editor;
        self.sci.call(SCI_SETTABWIDTH, style.tab_size, 0);
        self.sci.call(SCI_SETINDENT, style.indent_size, 0);
        self.sci.call(SCI_SETUSETABS, style.use_tabs.into(), 0);
    }

    /// Ctrl+Alt+S: pagina impostazioni; salvando si applicano subito a tutte le tab.
    fn open_settings(&self) {
        let current = self.settings.borrow().clone();
        let Some(changed) = settings_page::edit(self.hwnd, &self.palette, self.font.get(), &current, steroitor_core::shells::detect()) else { return };
        if changed == current {
            return;
        }
        changed.save();
        self.compare.set_font_fraction(changed.editor.font_size_fraction());
        *self.settings.borrow_mut() = changed;
        self.with_plugins(|plugins, _, settings, _| plugins.apply_settings(settings));
        self.remember_view();
        self.apply_indentation_all();
        // Riattiva la tab corrente (forzando il cambio) per ridare documento, stili e posizione alla vista.
        let active = self.active.replace(usize::MAX);
        self.activate(active);
    }

    /// Tab e indentazione su ogni documento del progetto attivo; poi va riattivata la tab corrente.
    fn apply_indentation_all(&self) {
        let pointers: Vec<isize> = self.docs.borrow().iter().map(|doc| doc.pointer).collect();
        for pointer in pointers {
            self.sci.call(SCI_SETDOCPOINTER, 0, pointer);
            self.apply_indentation();
        }
    }

    fn set_language(&self, language: &'static Language) {
        lang::set_lexer(&self.sci, language);
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
            docs.push(Doc { pointer, path, untitled, encoding, bom, eol, language, dirty: false, modified, stale: false, anchor: 0, caret: 0, first_line: 0, blame: Vec::new() });
            docs.len() - 1
        };
        self.active.set(index);
        self.sci.call(SCI_SETCODEPAGE, SC_CP_UTF8, 0);
        self.sci.call(SCI_SETEOLMODE, eol, 0);
        self.apply_indentation();
        self.set_language(language);
        self.apply_styles(language);
        self.sci.set_text(text.as_bytes());
        self.refresh();
        self.update_markers();
        self.with_plugins(|plugins, sci, settings, path| plugins.on_activate(sci, settings, path));
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
        self.with_plugins(|plugins, sci, settings, path| plugins.on_activate(sci, settings, path));
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
        self.show_blame_margin();
        if !stale {
            self.with_plugins(|plugins, sci, settings, path| plugins.on_activate(sci, settings, path));
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
        let removed = self.docs.borrow_mut().remove(index);
        if let Some(path) = &removed.path {
            self.plugins.close(path);
        }
        let pointer = removed.pointer;
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

    /// Chiede per le tab modificate del progetto attivo; true se si può procedere.
    fn confirm_project(&self) -> bool {
        let count = self.docs.borrow().len();
        (0..count).all(|index| self.confirm_discard(index))
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
        *self.panel.repos.borrow_mut() = roots.iter().map(|root| RepoTab { root: root.clone(), name: folder_name(Some(root)), changes: 0 }).collect();
        self.panel.has_folder.set(folder.is_some());
        *self.panel.root.borrow_mut() = None;
        *self.colors.borrow_mut() = FileColors::default();
        self.heads.borrow_mut().clear();
        for root in &roots {
            self.worker.refresh(root);
        }
        if let Some(folder) = &folder {
            let changes = Arc::clone(&self.changes.borrow());
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

    /// Un progetto parcheggiato nuovo, con explorer (nascosto) e Console suoi; con `pending` si carica alla prima apertura.
    fn park(&self, pending: Option<ProjectSession>) -> Parked {
        // SAFETY: modulo dell'eseguibile; l'explorer nasce nascosto finché il progetto non diventa attivo.
        let explorer = unsafe { Explorer::create(self.hwnd, GetModuleHandleW(std::ptr::null()), &self.palette) };
        explorer.set_font(self.font.get(), self.scale(TREE_ITEM_HEIGHT), self.scale(TREE_INDENT));
        explorer.set_image_list(self.explorer_icons.get());
        // SAFETY: controllo appena creato.
        unsafe { ShowWindow(explorer.hwnd(), SW_HIDE) };
        Parked { explorer, docs: Vec::new(), active: 0, repos: Vec::new(), colors: FileColors::default(), heads: HashMap::new(), watcher: None, changes: Changes::default(), panel: self.panel.new_state(), pending }
    }

    /// Carica nel progetto attivo, vuoto, cartella e file salvati.
    fn load_project(&self, project: ProjectSession) {
        self.set_folder(project.folder.filter(|folder| folder.is_dir()));
        for file in project.files.iter().filter(|file| file.is_file()) {
            self.open(file);
        }
        for tab in &project.unsaved {
            self.restore_unsaved(tab);
        }
        let count = self.docs.borrow().len();
        if count == 0 {
            self.new_document();
        } else {
            self.activate(project.active.min(count - 1));
        }
    }

    /// Rimette il testo di una tab modificata: sostituito al contenuto del file (o in una tab nuova), così risulta modificata e Ctrl+Z torna al file.
    fn restore_unsaved(&self, tab: &UnsavedTab) {
        let existing = tab.path.as_ref().and_then(|path| self.docs.borrow().iter().position(|doc| doc.path.as_ref() == Some(path)));
        match existing {
            Some(index) => self.activate(index),
            None => self.add_document(tab.path.clone(), "", tab.encoding.clone(), tab.bom),
        }
        let length = self.sci.call(SCI_GETLENGTH, 0, 0);
        self.sci.call(SCI_SETTARGETRANGE, 0, length);
        self.sci.call(SCI_REPLACETARGET, tab.content.len(), tab.content.as_ptr() as isize);
        let eol = eol_of(&tab.content);
        self.sci.call(SCI_SETEOLMODE, eol, 0);
        if let Some(doc) = self.docs.borrow_mut().get_mut(self.active.get()) {
            doc.eol = eol;
        }
    }

    /// Testo di un documento anche se non è quello mostrato.
    fn doc_text(&self, pointer: isize) -> String {
        if pointer == self.sci.call(SCI_GETDOCPOINTER, 0, 0) {
            return self.sci.text();
        }
        self.scratch.call(SCI_SETDOCPOINTER, 0, pointer);
        let text = self.scratch.text();
        self.scratch.call(SCI_SETDOCPOINTER, 0, 0);
        text
    }

    /// Ctrl+B (come IntelliJ): dichiarazione del simbolo sotto il cursore, dal server del plugin o cercandola nel file.
    fn go_to_declaration(&self) {
        let mut asked = false;
        self.with_plugins(|plugins, sci, settings, path| asked = plugins.definition(sci, settings, path));
        if !asked {
            self.local_declaration();
        }
    }

    fn local_declaration(&self) {
        let caret = self.sci.call(SCI_GETCURRENTPOS, 0, 0) as usize;
        let start = self.sci.call(SCI_WORDSTARTPOSITION, caret, 1) as usize;
        let end = self.sci.call(SCI_WORDENDPOSITION, caret, 1) as usize;
        let text = self.sci.text();
        let Some(word) = text.get(start..end).filter(|word| !word.is_empty()) else { return };
        if let Some(found) = plugins::local_declaration(&text, word) {
            self.sci.call(SCI_SETSEL, found, (found + word.len()) as isize);
            self.center_line(self.sci.call(SCI_LINEFROMPOSITION, found, 0).max(0) as usize);
        }
    }

    /// Apre il file e porta il cursore a (riga, colonna UTF-16), come le danno i server.
    fn navigate(&self, path: &Path, position: (usize, usize)) {
        self.open(path);
        if self.active_path().as_deref() != Some(path) {
            return;
        }
        let offset = plugins::offset_of(&self.sci.text(), position);
        self.sci.call(SCI_GOTOPOS, offset, 0);
        self.center_line(position.0);
        self.focus_editor();
    }

    /// Ctrl+P: un file del progetto per nome.
    fn quick_open(&self) {
        let Some(root) = self.explorer.root() else { return };
        let files = steroitor_core::fs::list_files(root.to_string_lossy().into_owned());
        if let Some(path) = quick_open::pick(self.hwnd, &self.palette, self.font.get(), &root, files) {
            self.open(&path);
        }
    }

    /// File e cartelle arrivati da un secondo avvio dell'app.
    fn open_paths(&self, paths: Vec<PathBuf>) {
        for path in paths {
            if path.is_dir() {
                self.open_project(path);
            } else if path.is_file() {
                self.open(&path);
            }
        }
    }

    /// Sposta la tab trascinata sulla posizione sotto il mouse; la tab trascinata resta quella attiva.
    fn drag_tab(&self, x: i32) {
        let Some(from) = self.tab_drag.get() else { return };
        let Some(to) = self.tabs.borrow().iter().position(|(tab, _)| x >= tab.left && x < tab.right) else { return };
        if to == from {
            return;
        }
        {
            let mut docs = self.docs.borrow_mut();
            let doc = docs.remove(from);
            docs.insert(to, doc);
        }
        self.active.set(to);
        self.tab_drag.set(Some(to));
        self.invalidate();
    }

    fn is_empty_project(&self) -> bool {
        self.explorer.root().is_none() && self.docs.borrow().iter().all(|doc| doc.path.is_none() && !doc.dirty)
    }

    /// La cartella va nel progetto che la ha già, in quello attivo se è vuoto, altrimenti in uno nuovo.
    fn open_project(&self, folder: PathBuf) {
        let key = crate::git::key(&folder);
        let same = |candidate: Option<PathBuf>| candidate.is_some_and(|candidate| crate::git::key(&candidate) == key);
        if same(self.explorer.root()) {
            return;
        }
        let existing = self.projects.borrow().iter().position(|parked| parked.as_ref().is_some_and(|parked| same(parked.folder())));
        if let Some(index) = existing {
            return self.switch_project(index);
        }
        if self.is_empty_project() {
            return self.set_folder(Some(folder));
        }
        let parked = self.park(Some(ProjectSession { folder: Some(folder), ..ProjectSession::default() }));
        let index = {
            let mut projects = self.projects.borrow_mut();
            projects.push(Some(parked));
            projects.len() - 1
        };
        self.switch_project(index);
    }

    /// Scambia i campi del progetto attivo con quelli del progetto `index`, che diventa attivo.
    fn switch_project(&self, index: usize) {
        if index == self.current.get() {
            return;
        }
        self.remember_view();
        let Some(mut parked) = self.projects.borrow_mut().get_mut(index).and_then(Option::take) else { return };
        self.explorer.swap(&parked.explorer);
        std::mem::swap(&mut *self.docs.borrow_mut(), &mut parked.docs);
        parked.active = self.active.replace(parked.active);
        std::mem::swap(&mut *self.repos.borrow_mut(), &mut parked.repos);
        std::mem::swap(&mut *self.colors.borrow_mut(), &mut parked.colors);
        std::mem::swap(&mut *self.heads.borrow_mut(), &mut parked.heads);
        std::mem::swap(&mut *self.watcher.borrow_mut(), &mut parked.watcher);
        std::mem::swap(&mut *self.changes.borrow_mut(), &mut parked.changes);
        self.panel.swap_state(&mut parked.panel);
        self.head_requests.borrow_mut().clear();
        let pending = parked.pending.take();
        // SAFETY: explorer del progetto uscente, ora parcheggiato.
        unsafe { ShowWindow(parked.explorer.hwnd(), SW_HIDE) };
        let previous = self.current.replace(index);
        self.projects.borrow_mut()[previous] = Some(parked);
        match pending {
            Some(project) => self.load_project(project),
            None => self.resume_project(),
        }
        self.layout();
        self.refresh();
    }

    /// Ritorno a un progetto già caricato: impostazioni dell'editor, tab attiva, stato Git e file cambiati nel frattempo.
    fn resume_project(&self) {
        self.apply_indentation_all();
        let active = self.active.replace(usize::MAX);
        self.activate(active);
        let roots: Vec<PathBuf> = self.repos.borrow().iter().map(|repo| repo.root.clone()).collect();
        for root in &roots {
            self.worker.refresh(root);
        }
        self.on_files_changed();
        self.sync_panel();
    }

    /// Chiude il progetto dopo aver chiesto per le tab modificate; l'ultimo progetto resta.
    fn close_project(&self, index: usize) {
        let count = self.projects.borrow().len();
        if count < 2 || index >= count {
            return;
        }
        self.switch_project(index);
        if !self.confirm_project() {
            return;
        }
        self.switch_project(if index + 1 < count { index + 1 } else { index - 1 });
        let Some(closed) = self.projects.borrow_mut().remove(index) else { return };
        if self.current.get() > index {
            self.current.set(self.current.get() - 1);
        }
        for doc in &closed.docs {
            if let Some(path) = &doc.path {
                self.plugins.close(path);
            }
            self.sci.call(SCI_RELEASEDOCUMENT, 0, doc.pointer);
        }
        self.panel.discard_state(closed.panel);
        // SAFETY: explorer del progetto chiuso, non più usato.
        unsafe { DestroyWindow(closed.explorer.hwnd()) };
        self.invalidate();
    }

    /// Il `+` della barra: sceglie una cartella e la apre in un progetto nuovo.
    fn new_project(&self) {
        if let Some(folder) = pick_folder(self.hwnd) {
            self.open_project(folder);
        }
    }

    fn on_files_changed(&self) {
        let (dirs, git_dirs) = self.changes.borrow().lock().map(|mut pending| std::mem::take(&mut *pending)).unwrap_or_default();
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
            Action::Create { dir, folder } => {
                let title = if folder { "Nuova cartella" } else { "Nuovo file" };
                let Some(name) = prompt(self.hwnd, &self.palette, self.font.get(), title, "Nome:", "") else { return };
                let created = if folder { steroitor_core::fs::create_dir(dir.to_string_lossy().into_owned(), name) } else { steroitor_core::fs::create_file(dir.to_string_lossy().into_owned(), name) };
                match created {
                    Ok(path) => {
                        let path = PathBuf::from(path);
                        self.explorer.refresh_dir(&dir);
                        self.explorer.reveal(&path);
                        if !folder {
                            self.open(&path);
                        }
                    }
                    Err(error) => {
                        message_box(self.hwnd, &error, MB_ICONERROR);
                    }
                }
            }
            Action::Rename(from) => {
                let name = folder_name(Some(&from));
                // Selezionato il nome fino all'estensione, come nella versione Tauri (tutto, per le cartelle).
                let selected = if from.is_dir() { name.encode_utf16().count() } else { name.rfind('.').filter(|&dot| dot > 0).map_or(name.encode_utf16().count(), |dot| name[..dot].encode_utf16().count()) };
                let Some(new) = prompt_selected(self.hwnd, &self.palette, self.font.get(), "Rinomina", "Nuovo nome:", &name, selected) else { return };
                if new == name {
                    return;
                }
                match steroitor_core::fs::rename_path(from.to_string_lossy().into_owned(), new) {
                    Ok(to) => {
                        let to = PathBuf::from(to);
                        if let Some(parent) = from.parent() {
                            self.explorer.refresh_dir(parent);
                        }
                        self.explorer.reveal(&to);
                        self.on_explorer_action(Action::Renamed { from, to });
                    }
                    Err(error) => {
                        message_box(self.hwnd, &error, MB_ICONERROR);
                    }
                }
            }
            Action::Blame(path) => {
                self.open(&path);
                if let Some(root) = repo_of(&self.repos.borrow(), &path).map(|repo| repo.root.clone()) {
                    self.worker.blame(&root, &path);
                }
            }
            Action::History(path) => self.show_file_history(&path, None),
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
            unsafe { SetFocus(self.explorer.hwnd()) };
        } else {
            self.focus_editor();
        }
    }

    fn toggle_wrap(&self) {
        self.wrap.set(!self.wrap.get());
        self.sci.call(SCI_SETWRAPMODE, if self.wrap.get() { SC_WRAP_WORD } else { 0 }, 0);
        self.minimap.sci.call(SCI_SETWRAPMODE, if self.wrap.get() { SC_WRAP_WORD } else { 0 }, 0);
    }

    fn toggle_minimap(&self) {
        self.minimap_on.set(!self.minimap_on.get());
        self.layout();
        self.minimap.sync(&self.sci);
    }

    /// Larghezza a destra dell'editor: striscia delle modifiche e minimap.
    fn right_strip(&self) -> i32 {
        self.scale(RULER) + if self.minimap_on.get() { self.scale(minimap::WIDTH) } else { 0 }
    }

    fn ruler_rect(&self) -> RECT {
        let client = self.client_rect();
        let top = self.tabs_bottom() + self.scale(self.find.height());
        let right = client.right - if self.minimap_on.get() { self.scale(minimap::WIDTH) } else { 0 };
        RECT { left: right - self.scale(RULER), top, right, bottom: self.panel_rect().top }
    }

    /// Porta la riga del documento al centro dell'editor, senza spostare il cursore.
    fn center_line(&self, line: usize) {
        let visible = self.sci.call(SCI_VISIBLEFROMDOCLINE, line, 0);
        let half = self.sci.call(SCI_LINESONSCREEN, 0, 0) / 2;
        self.sci.call(SCI_SETFIRSTVISIBLELINE, (visible - half).max(0) as usize, 0);
    }

    unsafe fn paint_ruler(&self, dc: HDC) {
        let rect = self.ruler_rect();
        fill(dc, &rect, self.palette.editor);
        fill(dc, &RECT { right: rect.left + 1, ..rect }, self.palette.border);
        let total = self.sci.call(SCI_GETLINECOUNT, 0, 0).max(1) as i32;
        let height = rect.bottom - rect.top;
        let mark = self.scale(3).max(2);
        for &(line, color) in self.ruler.borrow().iter() {
            let top = rect.top + (line as i32 * height / total).min(height - mark);
            fill(dc, &RECT { left: rect.left + self.scale(3), top, right: rect.right - self.scale(2), bottom: top + mark }, color);
        }
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
        if self.compare.owns(focus) {
            return self.compare.key(msg);
        }
        let key = msg.wParam as u16;
        if let Some(handled) = self.terminal_key(focus, msg, ctrl, shift) {
            return handled;
        }
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
        if ctrl && alt && key == 0x53 {
            self.open_settings();
            return true;
        }
        if ctrl && alt && key == 0x4B {
            self.commit(true);
            return true;
        }
        if focus == self.panel.message && ctrl && key == VK_RETURN {
            self.commit(false);
            return true;
        }
        if ctrl && !shift && key == 0x44 && (focus == self.panel.log.hwnd || focus == self.panel.files.hwnd) {
            self.show_diff(focus);
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
        if focus == self.explorer.hwnd() && !ctrl {
            match key {
                VK_F2 => {
                    if let Some(action) = self.explorer.rename_selected() {
                        self.on_explorer_action(action);
                    }
                }
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
        if key == VK_SPACE && !shift && focus == self.sci.hwnd {
            self.with_plugins(|plugins, sci, settings, path| plugins.complete(sci, settings, path, true));
            return true;
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
            (0x50, false) => self.quick_open(),
            (0x42, false) if focus == self.sci.hwnd => self.go_to_declaration(),
            (0x48, false) => self.open_find(true),
            (0x4B, true) => self.push(),
            (0x54, false) => self.new_terminal(),
            _ => return false,
        }
        true
    }

    /// Col focus in un terminale i tasti vanno alla shell, tranne le scorciatoie dell'app che lì servono:
    /// Alt+1/Z/`/9, Alt+F4, Ctrl+K (commit, con Shift push, con Alt commit e push), Ctrl+T, Ctrl+Tab. None le lascia all'app.
    fn terminal_key(&self, focus: HWND, msg: &MSG, ctrl: bool, shift: bool) -> Option<bool> {
        let index = self.panel.terminal_at(focus)?;
        let terminals = self.panel.terminals.borrow();
        let terminal = &terminals[index];
        let key = msg.wParam as u16;
        match msg.message {
            WM_CHAR => terminal.char(key, false),
            WM_SYSCHAR => terminal.char(key, true),
            WM_SYSKEYDOWN if matches!(key, 0x31 | 0x5A | 0xC0 | 0x39 | VK_F4) => return None,
            WM_SYSKEYDOWN => return Some(terminal.key_down(self.hwnd, key, ctrl, shift, true)),
            WM_KEYDOWN if ctrl && matches!(key, 0x4B | 0x54 | VK_TAB) => return None,
            WM_KEYDOWN => {
                // SAFETY: lettura dello stato della tastiera del thread.
                let alt = unsafe { GetKeyState(VK_MENU as i32) < 0 };
                return Some(terminal.key_down(self.hwnd, key, ctrl, shift, alt));
            }
            _ => return None,
        }
        Some(true)
    }

    /// Barra dei progetti: c'è solo con due o più progetti, come nella versione Tauri.
    fn bar_width(&self) -> i32 {
        if self.projects.borrow().len() >= 2 {
            self.scale(PROJECT_BAR)
        } else {
            0
        }
    }

    /// Barra dei progetti, a sinistra di tutto, per l'intera altezza.
    fn bar_rect(&self) -> RECT {
        RECT { left: 0, top: 0, right: self.bar_width(), bottom: self.client_rect().bottom }
    }

    fn main_top(&self) -> i32 {
        self.scale(TOP_BAR)
    }

    fn top_bar_rect(&self) -> RECT {
        RECT { left: self.bar_width(), top: 0, right: self.client_rect().right, bottom: self.main_top() }
    }

    /// Fondo della barra delle tab, dove inizia l'editor (o la barra di ricerca).
    fn tabs_bottom(&self) -> i32 {
        self.main_top() + self.scale(TAB_HEIGHT)
    }

    /// Bordo destro dell'explorer in pixel reali; con l'explorer nascosto, quello della barra dei progetti.
    fn explorer_edge(&self) -> i32 {
        if self.explorer_visible.get() {
            self.bar_width() + self.scale(self.explorer_width.get().max(EXPLORER_MIN))
        } else {
            self.bar_width()
        }
    }

    fn editor_left(&self) -> i32 {
        if self.explorer_visible.get() {
            self.explorer_edge() + self.scale(SPLITTER)
        } else {
            self.bar_width()
        }
    }

    fn layout(&self) {
        let client = self.client_rect();
        let tab = self.tabs_bottom();
        let panel = self.panel_rect();
        let bottom = panel.top;
        let left = self.editor_left();
        let find_height = self.scale(self.find.height());
        let tree_top = self.main_top() + self.scale(EXPLORER_HEADER);
        // SAFETY: finestre valide create da questo thread.
        unsafe {
            let tree = self.explorer_visible.get() && self.explorer.root().is_some();
            ShowWindow(self.explorer.hwnd(), if tree { SW_SHOW } else { SW_HIDE });
            MoveWindow(self.explorer.hwnd(), self.bar_width(), tree_top, self.explorer_edge() - self.bar_width(), (bottom - tree_top).max(0), 1);
            MoveWindow(self.sci.hwnd, left, tab + find_height, (client.right - left - self.right_strip()).max(0), (bottom - tab - find_height).max(0), 1);
            let minimap_width = self.scale(minimap::WIDTH);
            ShowWindow(self.minimap.sci.hwnd, if self.minimap_on.get() { SW_SHOW } else { SW_HIDE });
            MoveWindow(self.minimap.sci.hwnd, client.right - minimap_width, tab + find_height, minimap_width, (bottom - tab - find_height).max(0), 1);
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
        }
        self.find.layout(RECT { left, top: tab, right: client.right, bottom: tab + find_height }, |value| self.scale(value));
        self.panel.layout(panel, |value| self.scale(value));
        let mut tool: TTTOOLINFOW = unsafe { std::mem::zeroed() };
        tool.cbSize = std::mem::size_of::<TTTOOLINFOW>() as u32;
        tool.hwnd = self.hwnd;
        tool.rect = self.bar_rect();
        // SAFETY: struttura locale valida per la chiamata.
        unsafe { SendMessageW(self.tooltip, TTM_NEWTOOLRECTW, 0, &tool as *const _ as isize) };
    }

    fn invalidate(&self) {
        // SAFETY: finestra valida.
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    fn refresh(&self) {
        let title = {
            let docs = self.docs.borrow();
            docs.get(self.active.get()).map_or_else(|| "Steroitor".to_string(), |doc| format!("{}{} — Steroitor", if doc.dirty { "● " } else { "" }, doc.name()))
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
                self.with_plugins(|plugins, _, settings, path| plugins.on_modified(settings, path));
            }
            SCN_CHARADDED => self.with_plugins(|plugins, sci, settings, path| plugins.on_char(sci, settings, path, notification.ch)),
            SCN_AUTOCSELECTIONCHANGE => self.with_plugins(|plugins, _, settings, path| plugins.on_selection(settings, path, &notification.text())),
            SCN_AUTOCCANCELLED | SCN_AUTOCCOMPLETED => self.plugins.on_list_closed(&self.sci),
            SCN_DWELLSTART => self.with_plugins(|plugins, sci, settings, path| plugins.on_dwell(sci, settings, path, notification.position)),
            SCN_DWELLEND => self.plugins.on_dwell_end(&self.sci),
            SCN_MARGINRIGHTCLICK => {
                let mut point = POINT { x: 0, y: 0 };
                // SAFETY: posizione del cursore per il menu.
                unsafe { GetCursorPos(&mut point) };
                let line = self.sci.call(SCI_LINEFROMPOSITION, notification.position.max(0) as usize, 0);
                self.sci.call(SCI_GOTOLINE, line.max(0) as usize, 0);
                self.editor_menu(point, true);
            }
            SCN_MARGINCLICK => {
                let line = self.sci.call(SCI_LINEFROMPOSITION, notification.position.max(0) as usize, 0).max(0) as usize;
                match notification.margin as usize {
                    GIT_MARGIN => self.gutter_menu(line),
                    BLAME_MARGIN => self.open_blame_commit(line),
                    _ => {}
                }
            }
            SCN_UPDATEUI => {
                self.match_braces();
                if self.minimap_on.get() {
                    self.minimap.sync(&self.sci);
                }
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
        RECT { left: self.bar_width(), top: client.bottom - self.scale(STATUS_HEIGHT), ..client }
    }

    fn splitter_rect(&self) -> Option<RECT> {
        let edge = self.explorer_edge();
        self.explorer_visible.get().then(|| RECT { left: edge, top: self.main_top(), right: edge + self.scale(SPLITTER), bottom: self.panel_rect().top })
    }

    fn header_rect(&self) -> RECT {
        RECT { left: self.bar_width(), top: self.main_top(), right: self.explorer_edge(), bottom: self.main_top() + self.scale(EXPLORER_HEADER) }
    }

    fn find_rect(&self) -> RECT {
        let top = self.tabs_bottom();
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
            self.paint_projects(dc);
            self.paint_top_bar(dc);
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
            self.paint_ruler(dc);
            self.paint_status(dc);
            BitBlt(target, 0, 0, client.right, client.bottom, dc, 0, 0, SRCCOPY);
            SelectObject(dc, previous_font);
            SelectObject(dc, previous_bitmap);
            DeleteObject(bitmap);
            DeleteDC(dc);
            EndPaint(self.hwnd, &paint);
        }
    }

    /// Barra in alto: Nuovo, Apri, Apri cartella, Clona, Nuovo progetto … Impostazioni, Explorer, Pannello.
    unsafe fn paint_top_bar(&self, dc: HDC) {
        let palette = &self.palette;
        let rect = self.top_bar_rect();
        Self::strip(dc, rect, palette);
        let pointer = self.pointer.get();
        let mut buttons = Vec::new();
        let mut place = |dc: HDC, left: i32, label: &str, button: TopButton, on: bool| -> RECT {
            let area = RECT { left, top: rect.top + self.scale(4), right: left + text_width(dc, label) + self.scale(16), bottom: rect.bottom - self.scale(4) };
            if pointer.is_some_and(|(x, y)| contains(&area, x, y)) {
                fill(dc, &area, palette.hover);
            }
            text(dc, label, &area, if on { palette.accent } else { palette.fg }, DT_CENTER);
            buttons.push((area, button));
            area
        };
        let mut left = rect.left + self.scale(6);
        for (label, button) in [("Nuovo", TopButton::New), ("Apri", TopButton::Open), ("Apri cartella", TopButton::OpenFolder), ("Clona", TopButton::Clone), ("Nuovo progetto", TopButton::NewProject)] {
            left = place(dc, left, label, button, false).right + self.scale(2);
        }
        let entries = [("Impostazioni", TopButton::Settings, false), ("Explorer", TopButton::Explorer, self.explorer_visible.get()), ("Pannello", TopButton::Panel, self.panel.visible.get())];
        let width: i32 = entries.iter().map(|(label, _, _)| text_width(dc, label) + self.scale(18)).sum();
        let mut left = rect.right - self.scale(6) - width;
        for (label, button, on) in entries {
            left = place(dc, left, label, button, on).right + self.scale(2);
        }
        *self.top_buttons.borrow_mut() = buttons;
    }

    fn top_button_at(&self, x: i32, y: i32) -> Option<TopButton> {
        self.top_buttons.borrow().iter().find(|(rect, _)| contains(rect, x, y)).map(|(_, button)| *button)
    }

    fn on_top_button(&self, button: TopButton) {
        match button {
            TopButton::New => self.new_document(),
            TopButton::Open => open_files(self.hwnd).iter().for_each(|path| self.open(path)),
            TopButton::OpenFolder => self.open_folder(),
            TopButton::Clone => self.clone_repository(),
            TopButton::NewProject => self.new_project(),
            TopButton::Settings => self.open_settings(),
            TopButton::Explorer => self.toggle_explorer(),
            TopButton::Panel => {
                if self.panel.visible.get() {
                    self.panel.visible.set(false);
                    self.layout();
                    self.focus_editor();
                } else {
                    self.show_panel(self.panel.tab.get());
                }
            }
        }
        self.invalidate();
    }

    /// Intestazione dell'explorer: nome della cartella in maiuscolo e i pulsanti +, ⟳, ⊟, …; senza cartella il messaggio e due pulsanti.
    unsafe fn paint_header(&self, dc: HDC) {
        let palette = &self.palette;
        let rect = self.header_rect();
        let root = self.explorer.root();
        let pointer = self.pointer.get();
        let mut buttons = Vec::new();
        let mut entries = Vec::new();
        if root.is_some() {
            entries.extend([("+", HeaderButton::NewFile), ("⟳", HeaderButton::Refresh), ("⊟", HeaderButton::Collapse)]);
        }
        entries.push(("…", HeaderButton::OpenFolder));
        let mut right = rect.right - self.scale(4);
        for (label, button) in entries.iter().rev() {
            let area = RECT { left: right - text_width(dc, label) - self.scale(12), top: rect.top + self.scale(4), right, bottom: rect.bottom - self.scale(4) };
            if pointer.is_some_and(|(x, y)| contains(&area, x, y)) {
                fill(dc, &area, palette.hover);
            }
            text(dc, label, &area, palette.muted, DT_CENTER);
            buttons.push((area, *button));
            right = area.left;
        }
        let title = root.as_deref().map_or_else(|| "EXPLORER".to_string(), |root| folder_name(Some(root)).to_uppercase());
        let previous = SelectObject(dc, self.header_font.get());
        text(dc, &title, &RECT { left: rect.left + self.scale(12), right: right - self.scale(4), ..rect }, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
        SelectObject(dc, previous);
        if root.is_none() {
            let body = RECT { top: rect.bottom, bottom: self.panel_rect().top, ..rect };
            text(dc, "Nessuna cartella aperta.", &RECT { left: body.left + self.scale(12), top: body.top + self.scale(4), right: body.right - self.scale(12), bottom: body.top + self.scale(28) }, palette.muted, DT_LEFT);
            let mut top = body.top + self.scale(34);
            for (label, button) in [("Apri cartella", HeaderButton::OpenFolder), ("Clona repository…", HeaderButton::Clone)] {
                let area = RECT { left: body.left + self.scale(12), top, right: body.left + self.scale(12) + text_width(dc, label) + self.scale(24), bottom: top + self.scale(26) };
                if pointer.is_some_and(|(x, y)| contains(&area, x, y)) {
                    fill(dc, &area, palette.hover);
                }
                frame(dc, &area, palette.border);
                text(dc, label, &area, palette.fg, DT_CENTER);
                buttons.push((area, button));
                top = area.bottom + self.scale(6);
            }
        }
        *self.header_buttons.borrow_mut() = buttons;
    }

    fn header_button_at(&self, x: i32, y: i32) -> Option<HeaderButton> {
        if !self.explorer_visible.get() {
            return None;
        }
        self.header_buttons.borrow().iter().find(|(rect, _)| contains(rect, x, y)).map(|(_, button)| *button)
    }

    fn on_header_button(&self, button: HeaderButton) {
        match button {
            HeaderButton::NewFile => {
                if let Some(dir) = self.explorer.create_target() {
                    self.on_explorer_action(Action::Create { dir, folder: false });
                }
            }
            HeaderButton::Refresh => self.explorer.refresh_all(),
            HeaderButton::Collapse => self.explorer.collapse_all(),
            HeaderButton::OpenFolder => self.open_folder(),
            HeaderButton::Clone => self.clone_repository(),
        }
    }

    unsafe fn paint_tabs(&self, dc: HDC, client: &RECT) {
        let palette = &self.palette;
        let top = self.main_top();
        let bottom = self.tabs_bottom();
        let start = self.editor_left();
        Self::strip(dc, RECT { left: start, top, bottom, ..*client }, palette);
        let docs = self.docs.borrow();
        let mut rects = Vec::with_capacity(docs.len());
        let close = self.scale(CLOSE_SIZE);
        let pointer = self.pointer.get();
        let mut left = start;
        for (index, doc) in docs.iter().enumerate() {
            let name = doc.name();
            // Padding 0 6px 0 12px, 6px tra nome e ×, come nella versione Tauri.
            let width = (self.scale(12) + text_width(dc, &name) + self.scale(6) + close + self.scale(6)).min(self.scale(TAB_MAX_WIDTH));
            let tab = RECT { left, top, right: left + width, bottom };
            let active = index == self.active.get();
            let hovered = pointer.is_some_and(|(x, y)| contains(&tab, x, y));
            if active {
                fill(dc, &tab, palette.editor);
                fill(dc, &RECT { top: bottom - self.scale(2), ..tab }, palette.accent);
            }
            fill(dc, &RECT { left: tab.right - 1, top, right: tab.right, bottom: bottom - 1 }, palette.border);
            let close_rect = RECT { left: tab.right - self.scale(6) - close, top: (top + bottom - close) / 2, right: tab.right - self.scale(6), bottom: (top + bottom + close) / 2 };
            let label = RECT { left: tab.left + self.scale(12), right: close_rect.left - self.scale(6), ..tab };
            text(dc, &name, &label, if active { palette.fg } else { palette.muted }, DT_LEFT | DT_END_ELLIPSIS);
            let over_close = pointer.is_some_and(|(x, y)| contains(&close_rect, x, y));
            if over_close {
                fill(dc, &close_rect, palette.hover);
            }
            if doc.dirty && !over_close {
                text(dc, "●", &close_rect, palette.muted, DT_CENTER);
            } else if hovered || active || doc.dirty {
                text(dc, "×", &close_rect, palette.muted, DT_CENTER);
            }
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
        let padding = self.scale(8);
        // A sinistra, come nella versione Tauri: un branch per repository (col nome se sono più d'uno), poi l'operazione in corso.
        let mut vcs = Vec::new();
        let mut left = rect.left + self.scale(8);
        {
            let repos = self.repos.borrow();
            let selected = self.panel.root.borrow().clone().or_else(|| self.active_repo_root());
            let several = repos.len() > 1;
            for repo in repos.iter() {
                let Some(status) = repo.status.as_ref() else { continue };
                let branch = &status.branch;
                let name = branch.head.clone().unwrap_or_else(|| format!("HEAD staccato ({})", branch.oid.as_deref().and_then(|oid| oid.get(..7)).unwrap_or("?")));
                let ahead = if branch.ahead > 0 { format!(" ↑{}", branch.ahead) } else { String::new() };
                let behind = if branch.behind > 0 { format!(" ↓{}", branch.behind) } else { String::new() };
                let label = format!("⎇ {name}{ahead}{behind}");
                let prefix = if several { format!("{} ", folder_name(Some(&repo.root))) } else { String::new() };
                let width = text_width(dc, &prefix) + text_width(dc, &label);
                let area = RECT { left, right: left + width + padding * 2, ..rect };
                text(dc, &prefix, &RECT { left: area.left + padding, ..area }, palette.muted, DT_LEFT);
                text(dc, &label, &RECT { left: area.left + padding + text_width(dc, &prefix), ..area }, palette.fg, DT_LEFT);
                if several && selected.as_deref() == Some(repo.root.as_path()) {
                    fill(dc, &RECT { top: area.bottom - self.scale(2), ..area }, palette.accent);
                }
                vcs.push((area, repo.root.clone()));
                left = area.right + self.scale(4);
            }
            let operation = selected.as_deref().and_then(|root| repos.iter().find(|repo| repo.root == root)).and_then(|repo| repo.status.as_ref()).and_then(|status| {
                let conflicts = status.files.iter().filter(|file| file.status == FileStatus::Conflict).count();
                status.operation.as_ref().map(|operation| format!("{}{}", operation_name(operation).to_uppercase(), if conflicts > 0 { format!(" · {conflicts} conflitti") } else { String::new() }))
            });
            if let Some(operation) = operation {
                text(dc, &operation, &RECT { left: left + padding, ..rect }, palette.danger, DT_LEFT);
            }
        }
        *self.vcs_rects.borrow_mut() = vcs;
        // A destra: posizione, fine riga, encoding, linguaggio (con una tab aperta), Minimap, A capo.
        let docs = self.docs.borrow();
        let mut segments: Vec<(String, u32, u8)> = Vec::new();
        if let Some(doc) = docs.get(self.active.get()) {
            let caret = self.sci.call(SCI_GETCURRENTPOS, 0, 0);
            let line = self.sci.call(SCI_LINEFROMPOSITION, caret as usize, 0) + 1;
            let column = self.sci.call(SCI_GETCOLUMN, caret as usize, 0) + 1;
            let selected = (self.sci.call(SCI_GETSELECTIONEND, 0, 0) - self.sci.call(SCI_GETSELECTIONSTART, 0, 0)).max(0);
            let position = if selected > 0 { format!("Ln {line}, Col {column} ({selected} selezionati)") } else { format!("Ln {line}, Col {column}") };
            segments.push((position, palette.muted, 0));
            segments.push((eol_label(doc.eol).to_string(), palette.muted, 1));
            segments.push((format!("{}{}", doc.encoding, if doc.bom { " BOM" } else { "" }), palette.muted, 0));
            segments.push((doc.language.name.to_string(), palette.muted, 0));
        }
        segments.push(("Minimap".into(), if self.minimap_on.get() { palette.accent } else { palette.muted }, 2));
        segments.push(("A capo".into(), if self.wrap.get() { palette.accent } else { palette.muted }, 3));
        let mut right = rect.right - self.scale(8);
        for (label, color, kind) in segments.iter().rev() {
            let width = text_width(dc, label) + padding * 2;
            let area = RECT { left: right - width, right, ..rect };
            text(dc, label, &area, *color, DT_CENTER);
            match kind {
                1 => self.eol_rect.set(area),
                2 => self.minimap_rect.set(area),
                3 => self.wrap_rect.set(area),
                _ => {}
            }
            right = area.left - self.scale(4);
        }
        if docs.get(self.active.get()).is_none() {
            self.eol_rect.set(RECT { left: 0, top: 0, right: 0, bottom: 0 });
        }
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
        self.with_plugins(|plugins, sci, settings, path| plugins.highlight(sci, settings, path));
        self.sci.call(SCI_MARKERDELETEALL, usize::MAX, 0);
        self.ruler.borrow_mut().clear();
        // SAFETY: finestra valida; la striscia si ridisegna con i marker.
        unsafe { InvalidateRect(self.hwnd, &self.ruler_rect(), 0) };
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
        let palette = &self.palette;
        let mut ruler = self.ruler.borrow_mut();
        for (lines, marker, color) in [(changes.added, MARKER_ADDED, palette.git_added), (changes.modified, MARKER_MODIFIED, palette.git_modified), (changes.deleted, MARKER_DELETED, palette.danger)] {
            for line in lines {
                self.sci.call(SCI_MARKERADD, line, marker as isize);
                ruler.push((line, color));
            }
        }
    }

    fn on_git_events(&self) {
        for event in self.worker.take() {
            match event {
                Event::Status(root, Ok(status)) => {
                    let changes = status.files.iter().filter(|file| file.status != FileStatus::Ignored).count();
                    if let Some(tab) = self.panel.repos.borrow_mut().iter_mut().find(|tab| tab.root == root) {
                        tab.changes = changes;
                    }
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
                            self.load_log(&root);
                        }
                    }
                    if head_moved {
                        // Commit, checkout, reset: il contenuto in HEAD dei file di questo repository va riletto.
                        let prefix = crate::git::key(&root);
                        self.heads.borrow_mut().retain(|key, _| !key.starts_with(&prefix));
                        self.update_markers();
                    }
                    // SAFETY: controllo valido; ridisegno per i nuovi colori.
                    unsafe { InvalidateRect(self.explorer.hwnd(), std::ptr::null(), 0) };
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
                Event::Crlf(root, files, request) => {
                    let list = files.iter().take(10).map(|file| format!("  {file}")).collect::<Vec<_>>().join("\n");
                    let question = format!("Questi file in stage hanno fine riga CRLF:\n{list}\n\nCommittarli così?");
                    if message_box(self.hwnd, &question, MB_YESNO | MB_ICONWARNING) == IDYES {
                        self.worker.commit(&root, request, false);
                    } else {
                        self.worker.refresh(&root);
                    }
                }
                Event::Blame(file, result) => match result {
                    Ok(lines) => self.set_blame(&file, lines),
                    Err(error) => {
                        message_box(self.hwnd, &error, MB_ICONERROR);
                    }
                },
                Event::Cloned(target, result) => match result {
                    Ok(()) => self.open_project(target),
                    Err(error) => {
                        message_box(self.hwnd, &error, MB_ICONERROR);
                    }
                },
                Event::Committed(root, result) => {
                    match result {
                        Ok(()) => {
                            self.panel.amend.set(false);
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
                        self.load_log(&root);
                    }
                }
                Event::Branches(root, branches) => {
                    if let Some(repo) = self.repos.borrow_mut().iter_mut().find(|repo| repo.root == root) {
                        repo.branches = branches;
                    }
                    if self.panel.root.borrow().as_deref() == Some(root.as_path()) {
                        self.panel.set_branches(&root, self.branch_entries(&root));
                    }
                }
                Event::CommitFiles(root, hash, Ok(files)) => self.panel.set_commit_files(&root, &hash, files),
                Event::CommitFiles(_, _, Err(_)) => {}
                Event::Head(file, content) => {
                    let key = crate::git::key(&file);
                    self.head_requests.borrow_mut().remove(&key);
                    self.heads.borrow_mut().insert(key, content);
                    if self.active_path().as_deref() == Some(file.as_path()) {
                        self.update_markers();
                    }
                }
                Event::Compare(loaded) => self.compare.on_loaded(loaded),
                Event::Finished(root, result) => {
                    if let Err(error) = result {
                        message_box(self.hwnd, &error, MB_ICONERROR);
                    }
                    self.worker.refresh(&root);
                    if self.panel.visible.get() {
                        self.load_log(&root);
                    }
                }
            }
        }
        self.invalidate();
    }

    fn git_color(&self, param: LPARAM) -> Option<u32> {
        let path = self.explorer.path_of_param(param)?;
        let palette = &self.palette;
        if self.active_path().as_deref() == Some(path.as_path()) {
            return Some(palette.accent);
        }
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

    /// Menu VCS (clic sul branch nella status bar o Alt+`): operazione in corso, pull, push, fetch, stash, branch, checkout, merge, rebase.
    fn vcs_menu(&self) {
        if let Some(root) = self.active_repo_root() {
            self.vcs_menu_for(root);
        }
    }

    /// Righe (da 1) della selezione nell'editor, se c'è.
    fn selected_lines(&self) -> Option<(u32, u32)> {
        let (start, end) = (self.sci.call(SCI_GETSELECTIONSTART, 0, 0), self.sci.call(SCI_GETSELECTIONEND, 0, 0));
        if end <= start {
            return None;
        }
        let first = self.sci.call(SCI_LINEFROMPOSITION, start as usize, 0) + 1;
        let last = self.sci.call(SCI_LINEFROMPOSITION, (end - 1) as usize, 0) + 1;
        Some((first as u32, last as u32))
    }

    /// History del file (o delle righe) nella finestra di confronto.
    fn show_file_history(&self, path: &Path, lines: Option<(u32, u32)>) {
        let Some(root) = repo_of(&self.repos.borrow(), path).map(|repo| repo.root.clone()) else { return };
        self.compare.show_history(&root, path, lines, self.font.get());
    }

    /// Menu VCS come nella versione Tauri: operazione in corso, update, commit, push, fetch, history, branch, stash, metodo di update, branch locali e remoti.
    fn vcs_menu_for(&self, root: PathBuf) {
        let (branches, operation, current) = {
            let repos = self.repos.borrow();
            let repo = repos.iter().find(|repo| repo.root == root);
            let branches: Vec<(String, bool, bool)> = repo.map(|repo| repo.branches.iter().map(|branch| (branch.name.clone(), branch.remote, branch.current)).collect()).unwrap_or_default();
            let status = repo.and_then(|repo| repo.status.as_ref());
            let operation = status.and_then(|status| status.operation.as_ref().map(|operation| (operation_name(operation), status.files.iter().filter(|file| file.status == FileStatus::Conflict).count())));
            let current = status.and_then(|status| status.branch.head.clone()).unwrap_or_else(|| "HEAD".into());
            (branches, operation, current)
        };
        let path_text = root.to_string_lossy().into_owned();
        let stashes = self.worker.now(|git| git.stashes(&path_text)).unwrap_or_default();
        let rebase = self.settings.borrow().pull_rebase();
        let several = self.repos.borrow().len() > 1;
        let file = self.active_path().filter(|path| repo_of(&self.repos.borrow(), path).is_some_and(|repo| repo.root == root));
        let selection = file.as_ref().and_then(|_| self.selected_lines());
        let locals: Vec<String> = branches.iter().filter(|(_, remote, _)| !remote).map(|(name, _, _)| name.clone()).collect();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs() as i64);
        // SAFETY: menu creato, mostrato e distrutto qui (DestroyMenu distrugge anche i sottomenu); le stringhe vivono per tutta la chiamata.
        let command = unsafe {
            let mut strings: Vec<Vec<u16>> = Vec::new();
            let mut label = |text: &str| {
                strings.push(wide(text));
                strings.last().map_or(std::ptr::null(), |text| text.as_ptr())
            };
            let menu = CreatePopupMenu();
            if let Some((name, conflicts)) = operation {
                let title = format!("{}{} in corso · {conflicts} conflitti", name[..1].to_uppercase(), &name[1..]);
                AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, label(&title));
                AppendMenuW(menu, MF_STRING, VCS_CONTINUE, label(&format!("Continua {name}")));
                AppendMenuW(menu, MF_STRING, VCS_ABORT, label(&format!("Annulla {name}…")));
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            }
            AppendMenuW(menu, MF_STRING, VCS_PULL, label(&format!("Update\tCtrl+T · {}", if rebase { "Rebase" } else { "Merge" })));
            AppendMenuW(menu, MF_STRING, VCS_COMMIT, label("Commit…\tCtrl+K"));
            AppendMenuW(menu, MF_STRING, VCS_PUSH, label("Push…\tCtrl+Shift+K"));
            AppendMenuW(menu, MF_STRING, VCS_FETCH, label("Fetch"));
            if several {
                AppendMenuW(menu, MF_STRING, VCS_FETCH_ALL, label("Fetch di tutti i repository"));
            }
            if let Some(file) = &file {
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(menu, MF_STRING, VCS_HISTORY, label(&format!("History di {}", folder_name(Some(file)))));
                if let Some((first, last)) = selection {
                    AppendMenuW(menu, MF_STRING, VCS_HISTORY_SELECTION, label(&format!("History della selezione (righe {first}–{last})")));
                }
            }
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, VCS_NEW_BRANCH, label("Nuovo branch…"));
            AppendMenuW(menu, MF_STRING, VCS_STASH_PUSH, label("Stash modifiche…"));
            let stash_menu = CreatePopupMenu();
            if stashes.is_empty() {
                AppendMenuW(stash_menu, MF_STRING | MF_GRAYED, 0, label("Nessuno stash"));
            }
            for (index, stash) in stashes.iter().enumerate() {
                if index > 0 {
                    AppendMenuW(stash_menu, MF_SEPARATOR, 0, std::ptr::null());
                }
                AppendMenuW(stash_menu, MF_STRING | MF_GRAYED, 0, label(&format!("{}\t{}", stash.message, crate::panel::relative_time(stash.timestamp, now))));
                AppendMenuW(stash_menu, MF_STRING, VCS_STASH_POP + index, label("Pop (applica e rimuovi)"));
                AppendMenuW(stash_menu, MF_STRING, VCS_STASH_DROP + index, label("Elimina…"));
            }
            AppendMenuW(menu, MF_POPUP, stash_menu as usize, label(&format!("Stash salvati ({})", stashes.len())));
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, VCS_METHOD_MERGE, label(if rebase { "   Update con Merge" } else { "● Update con Merge" }));
            AppendMenuW(menu, MF_STRING, VCS_METHOD_REBASE, label(if rebase { "● Update con Rebase" } else { "   Update con Rebase" }));
            for (title, remote) in [("Branch locali", false), ("Branch remoti", true)] {
                let entries: Vec<usize> = branches
                    .iter()
                    .enumerate()
                    .filter(|(_, (name, is_remote, _))| *is_remote == remote && (!remote || !locals.iter().any(|local| name.split_once('/').is_some_and(|(_, rest)| rest == local))))
                    .map(|(index, _)| index)
                    .collect();
                if entries.is_empty() {
                    continue;
                }
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, label(title));
                for index in entries {
                    let (name, _, is_current) = &branches[index];
                    if *is_current {
                        AppendMenuW(menu, MF_STRING, 0, label(&format!("● {name}\tcorrente")));
                        continue;
                    }
                    let submenu = CreatePopupMenu();
                    AppendMenuW(submenu, MF_STRING | MF_GRAYED, 0, label(name));
                    AppendMenuW(submenu, MF_STRING, VCS_CHECKOUT + index, label("Checkout"));
                    AppendMenuW(submenu, MF_STRING, VCS_MERGE + index, label(&format!("Merge in {current}")));
                    AppendMenuW(submenu, MF_STRING, VCS_REBASE + index, label(&format!("Rebase {current} su {name}")));
                    AppendMenuW(submenu, MF_STRING, VCS_BRANCH_FROM + index, label("Nuovo branch da qui…"));
                    AppendMenuW(menu, MF_POPUP, submenu as usize, label(name));
                }
            }
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point);
            let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_BOTTOMALIGN | TPM_LEFTALIGN, point.x, point.y, 0, self.hwnd, std::ptr::null());
            DestroyMenu(menu);
            command as usize
        };
        let branch = |offset: usize| branches.get(command.wrapping_sub(offset)).map(|(name, remote, _)| (name.clone(), *remote));
        let stash = |offset: usize| stashes.get(command.wrapping_sub(offset)).map(|stash| stash.name.clone());
        match command {
            VCS_CONTINUE => self.worker.run(&root, |git, root| git.continue_operation(root)),
            VCS_ABORT => {
                if message_box(self.hwnd, "Annullare l'operazione in corso? Si torna allo stato di prima.", MB_YESNO | MB_ICONWARNING) == IDYES {
                    self.worker.run(&root, |git, root| git.abort_operation(root));
                }
            }
            VCS_PULL => self.worker.run(&root, move |git, root| git.pull(root, rebase)),
            VCS_COMMIT => {
                self.select_repo(root);
                self.show_panel(Tab::Commit);
            }
            VCS_PUSH => self.worker.run(&root, |git, root| git.push(root)),
            VCS_FETCH => self.worker.run(&root, |git, root| git.fetch(root)),
            VCS_FETCH_ALL => {
                let roots: Vec<PathBuf> = self.repos.borrow().iter().map(|repo| repo.root.clone()).collect();
                for root in roots {
                    self.worker.run(&root, |git, root| git.fetch(root));
                }
            }
            VCS_HISTORY => {
                if let Some(file) = &file {
                    self.show_file_history(file, None);
                }
            }
            VCS_HISTORY_SELECTION => {
                if let Some(file) = &file {
                    self.show_file_history(file, selection);
                }
            }
            VCS_STASH_PUSH => {
                if let Some(message) = prompt(self.hwnd, &self.palette, self.font.get(), "Stash modifiche", "Descrizione delle modifiche accantonate:", "WIP") {
                    self.worker.run(&root, move |git, root| git.stash_push(root, &message));
                }
            }
            VCS_NEW_BRANCH => {
                if let Some(name) = prompt(self.hwnd, &self.palette, self.font.get(), "Nuovo branch", "Nome del nuovo branch (parte da HEAD e diventa quello corrente):", "") {
                    self.worker.run(&root, move |git, root| git.create_branch(root, &name, true, None));
                }
            }
            VCS_METHOD_MERGE | VCS_METHOD_REBASE => {
                let mut settings = self.settings.borrow().clone();
                settings.update_method = (if command == VCS_METHOD_REBASE { "rebase" } else { "merge" }).into();
                settings.save();
                *self.settings.borrow_mut() = settings;
            }
            id if (VCS_CHECKOUT..VCS_MERGE).contains(&id) => {
                if let Some((name, remote)) = branch(VCS_CHECKOUT) {
                    self.worker.run(&root, move |git, root| git.switch(root, &name, remote));
                }
            }
            id if (VCS_MERGE..VCS_REBASE).contains(&id) => {
                if let Some((name, _)) = branch(VCS_MERGE) {
                    self.worker.run(&root, move |git, root| git.merge(root, &name));
                }
            }
            id if (VCS_REBASE..VCS_STASH_POP).contains(&id) => {
                if let Some((name, _)) = branch(VCS_REBASE) {
                    self.worker.run(&root, move |git, root| git.rebase(root, &name));
                }
            }
            id if (VCS_STASH_POP..VCS_STASH_DROP).contains(&id) => {
                if let Some(name) = stash(VCS_STASH_POP) {
                    self.worker.run(&root, move |git, root| git.stash_pop(root, &name));
                }
            }
            id if (VCS_STASH_DROP..VCS_BRANCH_FROM).contains(&id) => {
                if let Some(name) = stash(VCS_STASH_DROP) {
                    if message_box(self.hwnd, &format!("Eliminare {name}? Le modifiche accantonate andranno perse."), MB_YESNO | MB_ICONWARNING) == IDYES {
                        self.worker.run(&root, move |git, root| git.stash_drop(root, &name));
                    }
                }
            }
            id if id >= VCS_BRANCH_FROM => {
                if let Some((name, _)) = branch(VCS_BRANCH_FROM) {
                    if let Some(new) = prompt(self.hwnd, &self.palette, self.font.get(), "Nuovo branch", &format!("Nome del branch che parte da {name}:"), "") {
                        self.worker.run(&root, move |git, root| git.create_branch(root, &new, true, Some(&name)));
                    }
                }
            }
            _ => {}
        }
    }

    /// Menu dell'editor come nella versione Tauri; sul margine solo le voci Git.
    fn editor_menu(&self, screen: POINT, margin: bool) {
        let mut point = screen;
        // SAFETY: conversione nelle coordinate dell'editor.
        unsafe { ScreenToClient(self.sci.hwnd, &mut point) };
        let position = if screen.x == -1 && screen.y == -1 { self.sci.call(SCI_GETCURRENTPOS, 0, 0) } else { self.sci.call(SCI_POSITIONFROMPOINT, point.x as usize, point.y as isize) };
        let (start, end) = (self.sci.call(SCI_GETSELECTIONSTART, 0, 0), self.sci.call(SCI_GETSELECTIONEND, 0, 0));
        if !margin && (position < start || position > end) {
            self.sci.call(SCI_GOTOPOS, position.max(0) as usize, 0);
        }
        let has_selection = self.sci.call(SCI_GETSELECTIONEND, 0, 0) > self.sci.call(SCI_GETSELECTIONSTART, 0, 0);
        let caret = self.sci.call(SCI_GETCURRENTPOS, 0, 0) as usize;
        let (word_start, word_end) = (self.sci.call(SCI_WORDSTARTPOSITION, caret, 1) as usize, self.sci.call(SCI_WORDENDPOSITION, caret, 1) as usize);
        let word = self.sci.text().get(word_start..word_end).unwrap_or("").to_string();
        let file = self.active_path().filter(|path| repo_of(&self.repos.borrow(), path).is_some());
        let blame_on = self.docs.borrow().get(self.active.get()).is_some_and(|doc| !doc.blame.is_empty());
        let selection = self.selected_lines();
        let declaration = format!("Vai alla dichiarazione di {word}\tCtrl+B");
        let history_selection = selection.map(|(first, last)| format!("Mostra history della selezione (righe {first}–{last})"));
        let disabled = |enabled: bool, id: usize| if enabled { id } else { usize::MAX };
        let mut entries: Vec<(usize, &str)> = Vec::new();
        if !margin {
            entries.push(if word.is_empty() { (usize::MAX, "Vai alla dichiarazione") } else { (EDIT_DECLARATION, declaration.as_str()) });
            entries.extend([
                (0, ""),
                (disabled(has_selection, EDIT_CUT), "Taglia\tCtrl+X"),
                (disabled(has_selection, EDIT_COPY), "Copia\tCtrl+C"),
                (EDIT_PASTE, "Incolla\tCtrl+V"),
                (EDIT_SELECT_ALL, "Seleziona tutto\tCtrl+A"),
            ]);
        }
        if file.is_some() {
            if !margin {
                entries.push((0, ""));
            }
            entries.push((EDIT_BLAME, if blame_on { "Chiudi annotazioni" } else { "Annotate con Git Blame" }));
            entries.push((EDIT_HISTORY, "Mostra history del file"));
            entries.push(match &history_selection {
                Some(label) => (EDIT_HISTORY_SELECTION, label.as_str()),
                None => (usize::MAX, "Mostra history della selezione"),
            });
        }
        if entries.is_empty() {
            return;
        }
        match popup_menu(self.hwnd, &entries) {
            EDIT_DECLARATION => self.go_to_declaration(),
            EDIT_CUT => drop(self.sci.call(SCI_CUT, 0, 0)),
            EDIT_COPY => drop(self.sci.call(SCI_COPY, 0, 0)),
            EDIT_PASTE => drop(self.sci.call(SCI_PASTE, 0, 0)),
            EDIT_SELECT_ALL => drop(self.sci.call(SCI_SELECTALL, 0, 0)),
            EDIT_BLAME => {
                let Some(path) = file else { return };
                if blame_on {
                    self.set_blame(&path, Vec::new());
                } else if let Some(root) = repo_of(&self.repos.borrow(), &path).map(|repo| repo.root.clone()) {
                    self.worker.blame(&root, &path);
                }
            }
            EDIT_HISTORY => {
                if let Some(path) = file {
                    self.show_file_history(&path, None);
                }
            }
            EDIT_HISTORY_SELECTION => {
                if let Some(path) = file {
                    self.show_file_history(&path, selection);
                }
            }
            _ => {}
        }
    }

    /// Mostra il pannello sulla tab indicata, per il repository del file attivo.
    fn show_panel(&self, tab: Tab) {
        self.panel.visible.set(true);
        self.panel.tab.set(tab);
        let chosen = self.panel.root.borrow().clone().filter(|root| self.repos.borrow().iter().any(|repo| &repo.root == root));
        *self.panel.root.borrow_mut() = chosen.or_else(|| self.active_repo_root());
        self.sync_panel();
        self.layout();
        let focus = match tab {
            Tab::Console => self.panel.console_focus(),
            Tab::Git => self.panel.log.hwnd,
            Tab::Commit => self.panel.message,
        };
        // SAFETY: controllo valido.
        unsafe { SetFocus(focus) };
    }

    /// Ctrl+T fuori dai pannelli Git: un nuovo terminale nella cartella del progetto, con il focus.
    fn new_terminal(&self) {
        let shell = match steroitor_core::shells::resolve(Some(self.settings.borrow().terminal_shell.clone()).filter(|id| !id.is_empty()).as_deref()) {
            Ok(shell) => shell,
            Err(error) => {
                message_box(self.hwnd, &error, MB_ICONERROR);
                return;
            }
        };
        let id = self.panel.next_terminal.get();
        self.panel.next_terminal.set(id + 1);
        let cwd = self.explorer.root().or_else(|| self.active_repo_root());
        // SAFETY: istanza del modulo che ha creato la finestra principale.
        let instance = unsafe { GetWindowLongPtrW(self.hwnd, GWLP_HINSTANCE) } as HINSTANCE;
        match Terminal::open(self.hwnd, instance, &self.palette, id, &shell, cwd.as_deref()) {
            Ok(terminal) => self.panel.terminals.borrow_mut().push(terminal),
            Err(error) => {
                message_box(self.hwnd, &error, MB_ICONERROR);
                return;
            }
        }
        self.panel.view.set(Some(id));
        self.show_panel(Tab::Console);
    }

    fn close_terminal(&self, id: u32) {
        let mut terminals = self.panel.terminals.borrow_mut();
        let Some(index) = terminals.iter().position(|terminal| terminal.id == id) else { return };
        terminals.remove(index).close();
        if self.panel.view.get() == Some(id) {
            self.panel.view.set(terminals.get(index.min(terminals.len().saturating_sub(1))).map(|terminal| terminal.id));
        }
        drop(terminals);
        self.show_panel(Tab::Console);
    }

    fn on_console_target(&self, target: ConsoleTarget) {
        match target {
            ConsoleTarget::Log => {
                self.panel.view.set(None);
                self.show_panel(Tab::Console);
            }
            ConsoleTarget::Terminal(id) => {
                self.panel.view.set(Some(id));
                self.show_panel(Tab::Console);
            }
            ConsoleTarget::Close(id) => self.close_terminal(id),
            ConsoleTarget::New => self.new_terminal(),
        }
    }

    /// Anche i terminali dei progetti non attivi ricevono output e uscita.
    fn with_terminal(&self, id: u32, action: impl FnOnce(&Terminal)) {
        let current = self.panel.terminals.borrow();
        let projects = self.projects.borrow();
        let parked = projects.iter().flatten().flat_map(|parked| parked.panel.terminals.iter());
        if let Some(terminal) = current.iter().chain(parked).find(|terminal| terminal.id == id) {
            action(terminal);
        }
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

    fn branch_entries(&self, root: &Path) -> Vec<BranchEntry> {
        let repos = self.repos.borrow();
        let branches = repos.iter().find(|repo| repo.root == root).map(|repo| repo.branches.as_slice()).unwrap_or_default();
        branches.iter().map(|branch| BranchEntry { name: branch.name.clone(), remote: branch.remote, current: branch.current }).collect()
    }

    /// Tab di un repository nel pannello: Git e Commit mostrano quello.
    fn select_repo(&self, root: PathBuf) {
        if self.panel.root.borrow().as_deref() == Some(root.as_path()) {
            return;
        }
        *self.panel.root.borrow_mut() = Some(root);
        *self.panel.reference.borrow_mut() = None;
        self.panel.clear_details();
        self.panel.set_preview(None, "", "", &self.palette, self.settings.borrow().editor.font_size_fraction());
        self.sync_panel();
        self.layout();
    }

    fn sync_panel(&self) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        if let Some(status) = self.repos.borrow().iter().find(|repo| repo.root == root).and_then(|repo| repo.status.as_ref()) {
            self.panel.set_changes(&root, status);
        }
        self.panel.set_branches(&root, self.branch_entries(&root));
        if self.panel.tab.get() == Tab::Git {
            self.load_log(&root);
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
        let amend = self.panel.amend.get();
        let staged = self.panel.changes.borrow().iter().filter(|file| file.staged).count();
        if staged == 0 && !amend {
            message_box(self.hwnd, "Nessun file in stage: spunta i file da committare.", MB_ICONINFORMATION);
            return;
        }
        let detached = self.repos.borrow().iter().find(|repo| repo.root == root).and_then(|repo| repo.status.as_ref()).is_some_and(|status| status.branch.head.is_none());
        if detached && message_box(self.hwnd, "HEAD è staccato: il commit non starà su nessun branch e potresti perderlo cambiando branch. Committare comunque?", MB_YESNO | MB_ICONWARNING) != IDYES {
            return;
        }
        let path = root.to_string_lossy().into_owned();
        if amend && self.worker.now(|git| git.head_is_pushed(&path)).unwrap_or(false) && message_box(self.hwnd, "L'ultimo commit è già sul remote: dopo l'amend servirà un push forzato, da terminale. Continuare?", MB_YESNO | MB_ICONWARNING) != IDYES {
            return;
        }
        let warn_crlf = self.settings.borrow().warn_crlf.unwrap_or(true);
        self.worker.commit(&root, CommitRequest { files: Vec::new(), unstage: Vec::new(), message, push, amend }, warn_crlf);
    }

    /// Casella "Modifica l'ultimo commit": accesa con il messaggio vuoto, propone quello dell'ultimo commit.
    fn toggle_amend(&self) {
        let amend = !self.panel.amend.get();
        self.panel.amend.set(amend);
        let Some(root) = self.panel.root.borrow().clone() else { return };
        if amend && window_text(self.panel.message).trim().is_empty() {
            let path = root.to_string_lossy().into_owned();
            if let Ok(message) = self.worker.now(|git| git.last_commit_message(&path)) {
                let text = wide(&message.trim_end().replace("\r\n", "\n").replace('\n', "\r\n"));
                // SAFETY: controllo valido, stringa terminata da zero.
                unsafe { SetWindowTextW(self.panel.message, text.as_ptr()) };
            }
        }
    }

    fn pull(&self) {
        if let Some(root) = self.active_repo_root() {
            let rebase = self.settings.borrow().pull_rebase();
            self.worker.run(&root, move |git, root| git.pull(root, rebase));
        }
    }

    /// Menu contestuale di un commit nel log.
    fn log_menu(&self, index: usize) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let Some(hash) = self.panel.commits.borrow().get(index).map(|commit| commit.hash.clone()) else { return };
        let head = self.repos.borrow().iter().find(|repo| repo.root == root).and_then(|repo| repo.status.as_ref()).and_then(|status| status.branch.head.clone());
        let reset = format!("Reset {} qui", head.map_or_else(|| "HEAD".to_string(), |head| format!("\"{head}\"")));
        let entries = [
            (LOG_COPY_HASH, "Copia commit ID"),
            (LOG_SHOW_CHANGES, "Mostra modifiche\tCtrl+D"),
            (0, ""),
            (LOG_CHECKOUT, "Checkout (detached)"),
            (LOG_BRANCH, "Nuovo branch da qui…"),
            (LOG_TAG, "Crea tag…"),
            (LOG_REWORD, "Modifica messaggio…"),
            (0, ""),
            (LOG_CHERRY_PICK, "Cherry-pick"),
            (LOG_REVERT, "Revert"),
            (0, ""),
            (usize::MAX, reset.as_str()),
            (LOG_RESET_SOFT, "Soft (tiene le modifiche in stage)"),
            (LOG_RESET_MIXED, "Mixed (tiene le modifiche, fuori stage)"),
            (LOG_RESET_HARD, "Hard (scarta tutto)…"),
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
            LOG_REWORD => {
                let path = root.to_string_lossy().into_owned();
                let current = match self.worker.now(|git| git.commit_message(&path, &hash)) {
                    Ok(message) => message,
                    Err(error) => {
                        message_box(self.hwnd, &error, MB_ICONERROR);
                        return;
                    }
                };
                let label = format!("Nuovo messaggio di {short} (solo commit non ancora pushati):");
                if let Some(message) = prompt_text(self.hwnd, &self.palette, self.font.get(), "Modifica messaggio", &label, current.trim_end()) {
                    self.worker.run(&root, move |git, root| git.reword(root, &hash, &message));
                }
            }
            LOG_SHOW_CHANGES => self.show_commit_changes(index),
            _ => {}
        }
    }

    fn on_list_event(&self, list: HWND, event: ListEvent, index: usize) {
        if list == self.panel.branches.hwnd {
            return self.on_branch_event(event, index);
        }
        if list == self.panel.log.hwnd {
            match event {
                ListEvent::Select => self.select_commit(index),
                ListEvent::Activate => self.show_commit_changes(index),
                ListEvent::Menu => self.log_menu(index),
                ListEvent::Toggle => {}
            }
            return;
        }
        if list == self.panel.details.hwnd {
            let file = matches!(self.panel.detail_rows.borrow().get(index), Some(DetailRow::File(_)));
            if file && event == ListEvent::Activate {
                if let Some(commit) = self.panel.log.selected() {
                    self.show_commit_changes(commit);
                }
            }
            return;
        }
        if list != self.panel.files.hwnd {
            return;
        }
        match event {
            ListEvent::Toggle => self.toggle_stage(index),
            ListEvent::Select if self.panel.on_checkbox(|value| self.scale(value)) => {
                self.toggle_stage(index);
                self.preview_file(index);
            }
            ListEvent::Select => self.preview_file(index),
            ListEvent::Activate => self.show_file_diff(index),
            ListEvent::Menu => self.file_menu(index),
        }
        // SAFETY: area del pannello nella finestra principale (contatore dei file selezionati).
        unsafe { InvalidateRect(self.hwnd, &self.panel_rect(), 0) };
    }

    /// Log del repository con il branch scelto e i filtri sopra il log.
    fn load_log(&self, root: &Path) {
        let field = |hwnd: HWND| Some(window_text(hwnd).trim().to_string()).filter(|text| !text.is_empty());
        let (text, author, path) = (field(self.panel.filter_text), field(self.panel.filter_author), field(self.panel.filter_path));
        self.panel.graph_hidden.set(text.is_some() || author.is_some() || path.is_some());
        let reference = self.panel.reference.borrow().clone();
        self.worker.log(root, LogFilter { reference, author, text, path });
    }

    /// Clic su una riga dei branch: tutti, un branch (filtra il log) o una cartella (si apre o si chiude).
    fn on_branch_event(&self, event: ListEvent, index: usize) {
        let Some(row) = self.panel.branch_rows.borrow().get(index).cloned() else { return };
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let branch = |index: usize| self.panel.branch_list.borrow().get(index).map(|branch| (branch.name.clone(), branch.remote, branch.current));
        match (event, row) {
            (ListEvent::Select, BranchRow::All) => {
                *self.panel.reference.borrow_mut() = None;
                self.load_log(&root);
            }
            (ListEvent::Select, BranchRow::Branch { index, .. }) => {
                *self.panel.reference.borrow_mut() = branch(index).map(|(name, _, _)| name);
                self.load_log(&root);
            }
            (ListEvent::Select | ListEvent::Activate, BranchRow::Folder { key, .. }) => self.panel.toggle_folder(&key),
            (ListEvent::Menu, BranchRow::Branch { index, .. }) => {
                if let Some(branch) = branch(index) {
                    self.branch_menu(&root, branch);
                }
            }
            _ => {}
        }
        self.panel.branches.redraw();
    }

    /// Menu di un branch, come nella versione Tauri.
    fn branch_menu(&self, root: &Path, (name, remote, current): (String, bool, bool)) {
        let head = self.repos.borrow().iter().find(|repo| repo.root == root).and_then(|repo| repo.status.as_ref()).and_then(|status| status.branch.head.clone()).unwrap_or_else(|| "HEAD".into());
        let (merge, rebase) = (format!("Merge in {head}"), format!("Rebase {head} su {name}"));
        let mut entries = vec![(BRANCH_LOG, "Mostra log")];
        if !current {
            entries.extend([(0, ""), (BRANCH_CHECKOUT, "Checkout"), (BRANCH_MERGE, merge.as_str()), (BRANCH_REBASE, rebase.as_str())]);
        }
        entries.extend([(0, ""), (BRANCH_NEW, "Nuovo branch da qui…")]);
        match popup_menu(self.hwnd, &entries) {
            BRANCH_LOG => {
                *self.panel.reference.borrow_mut() = Some(name);
                self.load_log(root);
            }
            BRANCH_CHECKOUT => self.worker.run(root, move |git, root| git.switch(root, &name, remote)),
            BRANCH_MERGE => self.worker.run(root, move |git, root| git.merge(root, &name)),
            BRANCH_REBASE => self.worker.run(root, move |git, root| git.rebase(root, &name)),
            BRANCH_NEW => {
                if let Some(branch) = prompt(self.hwnd, &self.palette, self.font.get(), "Nuovo branch", &format!("Nome del branch che parte da {name}:"), "") {
                    self.worker.run(root, move |git, root| git.create_branch(root, &branch, true, Some(&name)));
                }
            }
            _ => {}
        }
    }

    /// Commit selezionato nel log: i suoi dettagli a destra, con i file cambiati.
    fn select_commit(&self, index: usize) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let had_details = self.panel.selected_commit.borrow().is_some();
        let hash = {
            let commits = self.panel.commits.borrow();
            let Some(commit) = commits.get(index) else { return };
            self.panel.show_details(commit);
            commit.hash.clone()
        };
        self.worker.commit_files(&root, hash);
        if !had_details {
            self.layout();
        }
    }

    /// Casella di un file: lo mette in stage, o lo toglie se è tutto in stage.
    fn toggle_stage(&self, index: usize) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let Some((path, unstage)) = self.panel.changes.borrow().get(index).map(|file| (file.path.clone(), file.staged && !file.unstaged)) else { return };
        if unstage {
            self.worker.run(&root, move |git, root| git.unstage(root, &[path]));
        } else {
            self.worker.run(&root, move |git, root| git.stage(root, &[path]));
        }
    }

    /// Casella in testa alla lista: tutto in stage, o tutto fuori se lo era già.
    fn toggle_stage_all(&self) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let (paths, unstage): (Vec<String>, bool) = {
            let changes = self.panel.changes.borrow();
            (changes.iter().map(|file| file.path.clone()).collect(), !changes.is_empty() && changes.iter().all(|file| file.staged && !file.unstaged))
        };
        if paths.is_empty() {
            return;
        }
        if unstage {
            self.worker.run(&root, move |git, root| git.unstage(root, &paths));
        } else {
            self.worker.run(&root, move |git, root| git.stage(root, &paths));
        }
    }

    /// Anteprima nella tab Commit: il file in HEAD e la copia di lavoro, affiancati.
    fn preview_file(&self, index: usize) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let Some((path, relative)) = self.panel.changes.borrow().get(index).map(|file| (file.path.clone(), file.relative.replace('\\', "/"))) else { return };
        let root_text = root.to_string_lossy().into_owned();
        let head = self.worker.now(|git| git.head_content(&root_text, &path)).ok().flatten().map(|file| file.content).unwrap_or_default();
        let current = std::fs::read(&path).map(|bytes| decode(&bytes).content).unwrap_or_default();
        self.panel.set_preview(Some(relative), &head, &current, &self.palette, self.settings.borrow().editor.font_size_fraction());
        self.invalidate();
    }

    /// Clic sul marker Git nel gutter: azioni sul blocco cambiato (se c'è) e annotazioni.
    fn gutter_menu(&self, line: usize) {
        let Some(path) = self.active_path() else { return };
        let Some(root) = repo_of(&self.repos.borrow(), &path).map(|repo| repo.root.clone()) else { return };
        let head = self.heads.borrow().get(&crate::git::key(&path)).cloned().flatten();
        let text = self.sci.text();
        let hunk = head.as_deref().and_then(|head| hunks::hunk_at(head, &text, line));
        let blame_on = self.docs.borrow().get(self.active.get()).is_some_and(|doc| !doc.blame.is_empty());
        let mut entries = Vec::new();
        if hunk.is_some() {
            entries.extend([(GUTTER_ROLLBACK, "Annulla le modifiche del blocco"), (GUTTER_STAGE, "Metti in stage il blocco"), (GUTTER_DIFF, "Mostra diff"), (0, "")]);
        }
        entries.push((GUTTER_BLAME, if blame_on { "Nascondi annotazioni" } else { "Annota (blame)" }));
        match popup_menu(self.hwnd, &entries) {
            GUTTER_ROLLBACK => {
                let (Some(head), Some(hunk)) = (head, hunk) else { return };
                self.rollback_hunk(&head, &text, &hunk);
            }
            GUTTER_STAGE => self.stage_hunk(&root, &path, &text, line),
            GUTTER_DIFF => self.compare.show_file(&root, &path, self.font.get()),
            GUTTER_BLAME if blame_on => self.set_blame(&path, Vec::new()),
            GUTTER_BLAME => self.worker.blame(&root, &path),
            _ => {}
        }
    }

    /// Riporta il blocco com'era in HEAD, con una modifica sola (annullabile con Ctrl+Z).
    fn rollback_hunk(&self, head: &str, text: &str, hunk: &hunks::Hunk) {
        let restored = hunks::rollback(head, text, hunk);
        let line_start = self.sci.call(SCI_POSITIONFROMLINE, hunk.new.start, 0);
        let start = if line_start < 0 { text.len() } else { (line_start as usize).min(text.len()) };
        let common = text.as_bytes().iter().zip(restored.as_bytes()).take(start).take_while(|(a, b)| a == b).count();
        let replacement = &restored.as_bytes()[common..];
        self.sci.call(SCI_SETTARGETRANGE, common, text.len() as isize);
        self.sci.call(SCI_REPLACETARGET, replacement.len(), replacement.as_ptr() as isize);
        self.update_markers();
    }

    /// Mette in stage solo il blocco sotto la riga: l'index prende quel blocco dal testo dell'editor, il file non cambia.
    fn stage_hunk(&self, root: &Path, path: &Path, text: &str, line: usize) {
        let (root_text, path_text) = (root.to_string_lossy().into_owned(), path.to_string_lossy().into_owned());
        let index = match self.worker.now(|git| git.index_content(&root_text, &path_text)) {
            Ok(index) => index.unwrap_or_default(),
            Err(error) => {
                message_box(self.hwnd, &error, MB_ICONERROR);
                return;
            }
        };
        let Some(hunk) = hunks::hunk_at(&index, text, line) else {
            message_box(self.hwnd, "Il blocco è già in stage.", MB_ICONINFORMATION);
            return;
        };
        let staged = hunks::apply(&index, text, &hunk);
        self.worker.run(root, move |git, root| git.stage_content(root, &path_text, &staged));
    }

    /// Annotazioni del file nel margine; con un elenco vuoto le toglie.
    fn set_blame(&self, file: &Path, lines: Vec<BlameLine>) {
        let Some(index) = self.docs.borrow().iter().position(|doc| doc.path.as_deref() == Some(file)) else { return };
        let empty = lines.is_empty();
        self.docs.borrow_mut()[index].blame = lines;
        if index != self.active.get() {
            return;
        }
        self.sci.call(SCI_MARGINTEXTCLEARALL, 0, 0);
        if !empty {
            let docs = self.docs.borrow();
            for (line, entry) in docs[index].blame.iter().enumerate() {
                self.sci.call_str(SCI_MARGINSETTEXT, line, &blame_label(entry));
                self.sci.call(SCI_MARGINSETSTYLE, line, STYLE_LINENUMBER as isize);
            }
        }
        self.show_blame_margin();
    }

    /// Larghezza del margine delle annotazioni per la tab attiva: zero se spente.
    fn show_blame_margin(&self) {
        let width = {
            let docs = self.docs.borrow();
            let longest = docs.get(self.active.get()).and_then(|doc| doc.blame.iter().map(blame_label).max_by_key(|label| label.chars().count()));
            longest.map_or(0, |label| self.sci.call_str(SCI_TEXTWIDTH, STYLE_LINENUMBER, &label) + self.scale(12) as isize)
        };
        self.sci.call(SCI_SETMARGINWIDTHN, BLAME_MARGIN, width);
    }

    /// Clic su un'annotazione: le modifiche del commit di quella riga.
    fn open_blame_commit(&self, line: usize) {
        let Some(path) = self.active_path() else { return };
        let Some(root) = repo_of(&self.repos.borrow(), &path).map(|repo| repo.root.clone()) else { return };
        let entry = self.docs.borrow().get(self.active.get()).and_then(|doc| doc.blame.get(line).cloned());
        if let Some(entry) = entry.filter(|entry| entry.hash.bytes().any(|byte| byte != b'0')) {
            self.compare.show_commit(&root, &entry.hash, &entry.summary, self.font.get());
        }
    }

    fn conflicted(&self, index: usize) -> bool {
        self.panel.changes.borrow().get(index).is_some_and(|file| file.status == FileStatus::Conflict)
    }

    fn open_change(&self, index: usize) {
        let path = self.panel.changes.borrow().get(index).map(|file| PathBuf::from(&file.path));
        if let Some(path) = path.filter(|path| path.is_file()) {
            self.open(&path);
        }
    }

    /// Diff del file rispetto a HEAD, o merge a 3 vie se è in conflitto.
    fn show_file_diff(&self, index: usize) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let Some(path) = self.panel.changes.borrow().get(index).map(|file| PathBuf::from(&file.path)) else { return };
        if self.conflicted(index) {
            self.compare.show_merge(&root, &path, self.font.get());
        } else {
            self.compare.show_file(&root, &path, self.font.get());
        }
    }

    fn show_commit_changes(&self, index: usize) {
        let Some(root) = self.panel.root.borrow().clone() else { return };
        let Some((hash, subject)) = self.panel.commits.borrow().get(index).map(|commit| (commit.hash.clone(), commit.subject.clone())) else { return };
        self.compare.show_commit(&root, &hash, &subject, self.font.get());
    }

    /// Ctrl+D col focus nel log (modifiche del commit) o nella lista del Commit (diff del file).
    fn show_diff(&self, focus: HWND) {
        if focus == self.panel.log.hwnd {
            if let Some(index) = self.panel.log.selected() {
                self.show_commit_changes(index);
            }
        } else if let Some(index) = self.panel.files.selected() {
            self.show_file_diff(index);
        }
    }

    fn file_menu(&self, index: usize) {
        let diff = if self.conflicted(index) { "Risolvi conflitti…" } else { "Mostra diff\tCtrl+D" };
        match popup_menu(self.hwnd, &[(FILE_DIFF, diff), (FILE_OPEN, "Apri nell'editor")]) {
            FILE_DIFF => self.show_file_diff(index),
            FILE_OPEN => self.open_change(index),
            _ => {}
        }
    }

    fn panel_height(&self) -> i32 {
        if !self.panel.visible.get() {
            return 0;
        }
        let available = self.status_rect().top - self.tabs_bottom() - self.scale(EDITOR_MIN);
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
        if contains(&self.panel_rect(), x, y) {
            if let Some(tab) = self.panel.tab_at(x, y) {
                return self.show_panel(tab);
            }
            if let Some(root) = self.panel.repo_at(x, y) {
                return self.select_repo(root);
            }
            if let Some(target) = self.panel.console_target_at(x, y) {
                return self.on_console_target(target);
            }
            match self.panel.button_at(x, y) {
                Some(panel::Button::Commit) => self.commit(false),
                Some(panel::Button::CommitAndPush) => self.commit(true),
                Some(panel::Button::Amend) => {
                    self.toggle_amend();
                    self.invalidate();
                }
                Some(panel::Button::StageAll) => self.toggle_stage_all(),
                Some(panel::Button::OpenInEditor) => {
                    let path = self.panel.changes.borrow().iter().find(|file| Some(file.relative.replace('\\', "/")) == *self.panel.preview_path.borrow()).map(|file| PathBuf::from(&file.path));
                    if let Some(path) = path.filter(|path| path.is_file()) {
                        self.open(&path);
                    }
                }
                Some(panel::Button::Continue) => {
                    if let Some(root) = self.panel.root.borrow().clone() {
                        self.worker.run(&root, |git, root| git.continue_operation(root));
                    }
                }
                Some(panel::Button::Abort) => {
                    let root = self.panel.root.borrow().clone();
                    if let Some(root) = root.filter(|_| message_box(self.hwnd, "Annullare l'operazione in corso? Si torna allo stato di prima.", MB_YESNO | MB_ICONWARNING) == IDYES) {
                        self.worker.run(&root, |git, root| git.abort_operation(root));
                    }
                }
                Some(panel::Button::Init) => self.init_repository(),
                Some(panel::Button::Clone) => self.clone_repository(),
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
        if let Some(index) = self.project_at(x, y) {
            return self.switch_project(index);
        }
        if let Some(button) = self.header_button_at(x, y) {
            return self.on_header_button(button);
        }
        if let Some(button) = self.top_button_at(x, y) {
            return self.on_top_button(button);
        }
        if contains(&self.wrap_rect.get(), x, y) {
            self.toggle_wrap();
            return self.invalidate();
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
        if contains(&self.minimap_rect.get(), x, y) {
            return self.toggle_minimap();
        }
        let ruler = self.ruler_rect();
        if contains(&ruler, x, y) {
            let total = self.sci.call(SCI_GETLINECOUNT, 0, 0).max(1);
            let line = (y - ruler.top) as isize * total / (ruler.bottom - ruler.top).max(1) as isize;
            return self.center_line(line.clamp(0, total - 1) as usize);
        }
        let vcs = self.vcs_rects.borrow().iter().find(|(rect, _)| contains(rect, x, y)).map(|(_, root)| root.clone());
        if let Some(root) = vcs {
            return self.vcs_menu_for(root);
        }
        match self.tab_at(x, y) {
            Some((index, true)) => self.close(index),
            Some((index, false)) => {
                self.activate(index);
                self.tab_drag.set(Some(index));
                // SAFETY: cattura del mouse sulla finestra principale fino al rilascio.
                unsafe { SetCapture(self.hwnd) };
            }
            None => {}
        }
    }

    /// Chiede URL e cartella di destinazione; a clone finito la cartella si apre come progetto.
    fn clone_repository(&self) {
        let Some(url) = prompt(self.hwnd, &self.palette, self.font.get(), "Clona repository", "URL del repository (HTTPS o SSH):", "") else { return };
        let Some(name) = repo_name(&url) else {
            message_box(self.hwnd, "URL non valido.", MB_ICONERROR);
            return;
        };
        let Some(parent) = pick_folder(self.hwnd) else { return };
        let target = parent.join(&name);
        if target.exists() {
            message_box(self.hwnd, &format!("{} esiste già.", target.display()), MB_ICONERROR);
            return;
        }
        self.worker.clone_repo(url.trim().to_string(), target);
        self.show_panel(Tab::Console);
        self.panel.view.set(None);
    }

    /// `git init` nella cartella del progetto, poi la si rilegge per trovare il repository.
    fn init_repository(&self) {
        let Some(folder) = self.explorer.root() else { return };
        let path = folder.to_string_lossy().into_owned();
        match self.worker.now(|git| git.init_repo(&path)) {
            Ok(()) => self.set_folder(Some(folder)),
            Err(error) => {
                message_box(self.hwnd, &error, MB_ICONERROR);
            }
        }
    }

    /// Barra dei progetti: un quadrato con l'iniziale per progetto, quello attivo pieno, poi il `+`.
    unsafe fn paint_projects(&self, dc: HDC) {
        let palette = &self.palette;
        let bar = self.bar_rect();
        if bar.right == 0 {
            self.project_rects.borrow_mut().clear();
            return;
        }
        fill(dc, &bar, palette.panel);
        fill(dc, &RECT { left: bar.right - 1, ..bar }, palette.border);
        let size = self.scale(PROJECT_BUTTON);
        let left = (bar.right - size) / 2;
        let current = self.current.get();
        let folders: Vec<Option<PathBuf>> = self
            .projects
            .borrow()
            .iter()
            .enumerate()
            .map(|(index, parked)| if index == current { self.explorer.root() } else { parked.as_ref().and_then(Parked::folder) })
            .collect();
        let mut rects = Vec::with_capacity(folders.len());
        let pointer = self.pointer.get();
        for (index, folder) in folders.iter().enumerate() {
            let top = self.scale(6) + index as i32 * (size + self.scale(4));
            let rect = RECT { left, top, right: left + size, bottom: top + size };
            if index == current || pointer.is_some_and(|(x, y)| contains(&rect, x, y)) {
                fill(dc, &rect, palette.hover);
            }
            if index == current {
                fill(dc, &RECT { left: rect.left, right: rect.left + self.scale(2), ..rect }, palette.accent);
            }
            let icon = self.scale(20);
            DrawIconEx(dc, rect.left + (size - icon) / 2, rect.top + (size - icon) / 2, self.app_icon, icon, icon, 0, std::ptr::null_mut(), DI_NORMAL);
            let initial = folder_initial(folder.as_deref());
            let previous = SelectObject(dc, self.badge_font.get());
            let badge_width = (text_width(dc, &initial) + self.scale(4)).max(self.scale(13));
            let badge = RECT { left: rect.right - badge_width, top: rect.bottom - self.scale(13), right: rect.right, bottom: rect.bottom };
            fill(dc, &badge, palette.accent);
            text(dc, &initial, &badge, 0xFFFFFF, DT_CENTER);
            SelectObject(dc, previous);
            rects.push(rect);
        }
        *self.project_rects.borrow_mut() = rects;
    }

    fn project_at(&self, x: i32, y: i32) -> Option<usize> {
        self.project_rects.borrow().iter().position(|rect| contains(rect, x, y))
    }

    fn on_mouse_move(&self, x: i32, y: i32) {
        if self.dragging_panel.get() {
            let height = self.status_rect().top - y;
            self.panel.height.set(height * 96 / self.dpi().max(1));
            return self.layout();
        }
        if self.tab_drag.get().is_some() {
            return self.drag_tab(x);
        }
        if !self.dragging.get() {
            self.track_pointer(x, y);
            return self.hover_project(x, y);
        }
        let client = self.client_rect();
        let max = (client.right - self.bar_width() - self.scale(EDITOR_MIN)).max(self.scale(EXPLORER_MIN));
        let width = (x - self.bar_width()).clamp(self.scale(EXPLORER_MIN), max);
        self.explorer_width.set(width * 96 / self.dpi().max(1));
        self.layout();
    }

    /// Effetti al passaggio del mouse: si ridisegnano le barre quando cambia l'elemento sotto il puntatore.
    fn track_pointer(&self, x: i32, y: i32) {
        let target = |point: Option<(i32, i32)>| point.map(|(x, y)| (self.top_button_at(x, y).map(|button| button as usize), self.tab_at(x, y), self.project_at(x, y), self.header_button_at(x, y).map(|button| button as usize)));
        let before = target(self.pointer.replace(Some((x, y))));
        if !self.tracking.replace(true) {
            let mut track = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: self.hwnd, dwHoverTime: 0 };
            // SAFETY: struttura locale valida per la chiamata.
            unsafe { TrackMouseEvent(&mut track) };
        }
        if before != target(Some((x, y))) {
            self.invalidate();
        }
    }

    /// Il tooltip della barra mostra il nome del progetto sotto il mouse.
    fn hover_project(&self, x: i32, y: i32) {
        let hovered = self.project_rects.borrow().iter().position(|rect| contains(rect, x, y));
        if hovered == self.hovered.replace(hovered) {
            return;
        }
        let label = match hovered {
            None => String::new(),
            Some(index) => {
                let folder = if index == self.current.get() { self.explorer.root() } else { self.projects.borrow()[index].as_ref().and_then(Parked::folder) };
                folder.map_or_else(|| folder_name(None), |folder| folder.display().to_string())
            }
        };
        let mut text = wide(&label);
        // SAFETY: struttura e testo locali, validi per la chiamata.
        unsafe {
            let mut tool: TTTOOLINFOW = std::mem::zeroed();
            tool.cbSize = std::mem::size_of::<TTTOOLINFOW>() as u32;
            tool.hwnd = self.hwnd;
            tool.lpszText = text.as_mut_ptr();
            SendMessageW(self.tooltip, TTM_POP, 0, 0);
            SendMessageW(self.tooltip, TTM_UPDATETIPTEXTW, 0, &tool as *const _ as isize);
        }
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
                self.open_project(path);
            } else {
                self.open(&path);
            }
        }
    }

    fn save_session(&self) {
        let projects: Vec<ProjectSession> = self
            .projects
            .borrow()
            .iter()
            .map(|parked| match parked {
                Some(parked) => parked.session(|pointer| self.doc_text(pointer)),
                None => session_of(self.explorer.root(), &self.docs.borrow(), self.active.get(), |pointer| self.doc_text(pointer)),
            })
            .collect();
        // SAFETY: lettura della posizione della finestra.
        let (window, maximized) = unsafe {
            let mut placement: WINDOWPLACEMENT = std::mem::zeroed();
            placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
            GetWindowPlacement(self.hwnd, &mut placement);
            let normal = placement.rcNormalPosition;
            (Some([normal.left, normal.top, normal.right, normal.bottom]), IsZoomed(self.hwnd) != 0)
        };
        Session {
            projects,
            active_project: self.current.get(),
            explorer_width: Some(self.explorer_width.get()),
            explorer_visible: Some(self.explorer_visible.get()),
            panel_visible: Some(self.panel.visible.get()),
            panel_height: Some(self.panel.height.get()),
            panel_tab: Some(match self.panel.tab.get() {
                Tab::Console => "console",
                Tab::Git => "git",
                Tab::Commit => "commit",
            }.into()),
            window,
            maximized,
            wrap: self.wrap.get(),
            minimap: self.minimap_on.get(),
            ..Session::default()
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
        let clickable = contains(&self.eol_rect.get(), point.x, point.y) || contains(&self.minimap_rect.get(), point.x, point.y) || contains(&self.wrap_rect.get(), point.x, point.y) || self.vcs_rects.borrow().iter().any(|(rect, _)| contains(rect, point.x, point.y)) || self.project_at(point.x, point.y).is_some() || self.header_button_at(point.x, point.y).is_some() || self.top_button_at(point.x, point.y).is_some();
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
        WM_MOUSELEAVE => {
            app.tracking.set(false);
            if app.pointer.take().is_some() {
                app.invalidate();
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            // Doppio clic sullo spazio vuoto della barra delle tab: un file nuovo, come nella versione Tauri.
            let (x, y) = point(lparam);
            let strip = y >= app.main_top() && y < app.tabs_bottom() && x >= app.editor_left();
            if strip && app.tab_at(x, y).is_none() {
                app.new_document();
            } else {
                app.on_click(x, y);
            }
            0
        }
        WM_LBUTTONUP => {
            if app.dragging.replace(false) | app.dragging_panel.replace(false) | app.tab_drag.take().is_some() {
                ReleaseCapture();
            }
            0
        }
        WM_MBUTTONUP => {
            let (x, y) = point(lparam);
            if let Some(index) = app.project_at(x, y) {
                app.close_project(index);
            } else if let Some((index, _)) = app.tab_at(x, y) {
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
            if app.panel.preview_pane(header.hwndFrom) {
                if header.code == SCN_UPDATEUI {
                    crate::side_by_side::sync_scroll([&app.panel.preview[0], &app.panel.preview[1]], header.hwndFrom);
                }
                return 0;
            }
            if header.hwndFrom == app.explorer.hwnd() && header.code == NM_CUSTOMDRAW {
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
            if header.hwndFrom == app.explorer.hwnd() {
                let (result, action) = {
                    let repos = app.repos.borrow();
                    let in_repo = |path: &Path| repo_of(&repos, path).is_some();
                    app.explorer.on_notify(hwnd, header, lparam, &in_repo)
                };
                if let Some(action) = action {
                    app.on_explorer_action(action);
                }
                return result;
            }
            0
        }
        WM_COMMAND => {
            let notification = ((wparam >> 16) & 0xFFFF) as u32;
            if app.panel.is_filter(lparam as HWND) && notification == EN_CHANGE {
                SetTimer(hwnd, LOG_FILTER_TIMER, LOG_FILTER_DELAY_MS, None);
            }
            if lparam as HWND == app.panel.message && notification == EN_CHANGE {
                InvalidateRect(hwnd, &app.panel_rect(), 0);
            }
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
        WM_CONTEXTMENU if wparam as HWND == app.sci.hwnd => {
            let point = POINT { x: (lparam & 0xFFFF) as i16 as i32, y: ((lparam >> 16) & 0xFFFF) as i16 as i32 };
            app.editor_menu(point, false);
            0
        }
        crate::compare::WM_COMPARE_OPEN => {
            if let Some(path) = app.compare.take_open_request() {
                app.open(&path);
                // Apre alla prima modifica, come "Apri nell'editor" della versione Tauri.
                app.update_markers();
                let line = app.sci.call(SCI_MARKERNEXT, 0, 0b111);
                if line >= 0 {
                    app.sci.call(SCI_GOTOLINE, line as usize, 0);
                    app.center_line(line as usize);
                }
            }
            0
        }
        WM_DROPFILES => {
            app.on_drop(wparam as HDROP);
            0
        }
        WM_GIT_EVENT => {
            app.on_git_events();
            0
        }
        WM_TERMINAL_OUTPUT => {
            app.with_terminal(wparam as u32, Terminal::on_output);
            0
        }
        WM_TERMINAL_EXIT => {
            app.with_terminal(wparam as u32, |terminal| terminal.on_exit(lparam as u32));
            app.invalidate();
            0
        }
        WM_COMMAND_LOGGED => {
            app.panel.append_console(&app.worker.take_log());
            0
        }
        WM_LIST_PAINT => {
            let row = &*(lparam as *const RowPaint);
            let (palette, font, scale) = (&app.palette, app.font.get(), |value| app.scale(value));
            if row.list == app.panel.log.hwnd {
                app.panel.paint_log_row(row, palette, font, scale);
            } else if row.list == app.panel.branches.hwnd {
                app.panel.paint_branch_row(row, palette, font, scale);
            } else if row.list == app.panel.details.hwnd {
                app.panel.paint_detail_row(row, palette, font, scale);
            } else {
                app.panel.paint_file_row(row, palette, font, scale);
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
        WM_TIMER if wparam == LOG_FILTER_TIMER => {
            KillTimer(hwnd, LOG_FILTER_TIMER);
            if let Some(root) = app.panel.root.borrow().clone() {
                app.load_log(&root);
            }
            0
        }
        WM_TIMER if wparam == plugins::COMPLETION_TIMER => {
            app.with_plugins(|plugins, sci, settings, path| plugins.complete(sci, settings, path, false));
            0
        }
        WM_TIMER if wparam == plugins::CHECK_TIMER => {
            app.with_plugins(|plugins, sci, settings, path| plugins.check(sci, settings, path));
            0
        }
        plugins::WM_PLUGIN_EVENT => {
            let mut problems = Vec::new();
            app.with_plugins(|plugins, sci, settings, path| problems = plugins.on_event(sci, settings, path));
            app.panel.append_console(&problems);
            match app.plugins.take_navigation() {
                Some(Some((path, position))) => app.navigate(&path, position),
                Some(None) => app.local_declaration(),
                None => {}
            }
            0
        }
        WM_MINIMAP_LINE => {
            app.center_line(wparam);
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
            app.save_session();
            DestroyWindow(hwnd);
            0
        }
        WM_COPYDATA => {
            let data = &*(lparam as *const windows_sys::Win32::System::DataExchange::COPYDATASTRUCT);
            if data.dwData == COPYDATA_OPEN && !data.lpData.is_null() {
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(data.lpData as *const u16, data.cbData as usize / 2));
                app.open_paths(text.lines().filter(|line| !line.is_empty()).map(PathBuf::from).collect());
            }
            1
        }
        WM_DESTROY => {
            app.panel.terminals.borrow_mut().drain(..).for_each(|terminal| terminal.close());
            for parked in app.projects.borrow_mut().iter_mut().flatten() {
                parked.panel.terminals.drain(..).for_each(|terminal| terminal.close());
            }
            app.plugins.shutdown();
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

//! Pannello inferiore a tab, come nella versione Tauri: Console (comandi git e terminali), Git (branch, filtri, log
//! con grafo, dettagli del commit), Commit (file con lo stage vero, messaggio, anteprima delle modifiche).
//! Con più repository nella cartella, Git e Commit hanno sopra una tab per repository.
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use steroitor_core::git::graph::{layout_graph, GraphRow, Half};
use steroitor_core::git::parse::{Commit, FileStatus, RepoStatus};
use steroitor_core::git::runner::CommandLog;
use windows_sys::Win32::Foundation::{FILETIME, HINSTANCE, HWND, POINT, RECT, SYSTEMTIME};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::list::{List, RowPaint};
use crate::paint::{fill, frame, text, text_width};
use crate::sci::*;
use crate::terminal::Terminal;
use crate::theme::Palette;

pub const HEADER: i32 = 28;
const REPO_ROW: i32 = 30;
const FILTER_ROW: i32 = 32;
const BRANCHES_WIDTH: i32 = 180;
const DETAILS_WIDTH: i32 = 320;
const MESSAGE_HEIGHT: i32 = 90;
const BUTTON_ROW: i32 = 40;
const LIST_HEADER: i32 = 28;
const LANE: i32 = 14;
const CHECKBOX: i32 = 14;
const SCI_SETREADONLY: u32 = 2171;
const SCI_SETUNDOCOLLECTION: u32 = 2012;
const SCI_DOCUMENTEND: u32 = 2318;
const SCI_STARTSTYLING: u32 = 2032;
const SCI_SETSTYLING: u32 = 2033;
/// Righe della Console: sotto-tab (Comandi Git, terminali, + Terminale) e barra del log dei comandi.
const CONSOLE_ROW: i32 = 26;
/// Voci del log dei comandi tenute per progetto.
const LOG_LIMIT: usize = 500;
const STYLE_DANGER: usize = 1;
const STYLE_MUTED: usize = 2;
/// Colori delle corsie del grafo (0xRRGGBB), a giro.
const LANE_COLORS: [u32; 8] = [0x5a9cf8, 0x73c991, 0xd5b778, 0xc77dbb, 0xd9776e, 0x2aacb8, 0xcf8e6d, 0x9da0a8];

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Console,
    Git,
    Commit,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Button {
    Commit,
    CommitAndPush,
    Amend,
    StageAll,
    OpenInEditor,
    Continue,
    Abort,
    Init,
    Clone,
    /// Barra del log dei comandi: casella "Comandi in background", Copia, Pulisci.
    Background,
    CopyLog,
    ClearLog,
}

/// Sotto-tab della Console: il log dei comandi git, un terminale (o la sua ×), il pulsante per aprirne uno.
#[derive(Clone, Copy, PartialEq)]
pub enum ConsoleTarget {
    Log,
    Terminal(u32),
    Close(u32),
    New,
    /// La freccia accanto a "+ Terminale": il menu delle shell.
    Shells,
}

pub struct ChangedFile {
    pub path: String,
    pub relative: String,
    pub status: FileStatus,
    /// Modifiche nell'index e nel working tree: le due insieme fanno la casella a metà.
    pub staged: bool,
    pub unstaged: bool,
}

pub struct BranchEntry {
    pub name: String,
    pub remote: bool,
    pub current: bool,
}

/// Riga della lista dei branch: "Tutti i branch", un titolo, una cartella (nome con `/`) o un branch.
#[derive(Clone, PartialEq, Debug)]
pub enum BranchRow {
    All,
    Header(&'static str),
    Folder { key: String, name: String, depth: usize },
    Branch { index: usize, name: String, depth: usize },
}

/// Riga dei dettagli del commit selezionato.
#[derive(Clone)]
pub enum DetailRow {
    Subject(String),
    Body(String),
    Field(&'static str, String),
    Files(String),
    File(usize),
}

pub struct CommitFile {
    pub status: String,
    pub relative: String,
}

/// Tab di un repository della cartella: radice, nome, numero di file cambiati.
#[derive(Clone)]
pub struct RepoTab {
    pub root: PathBuf,
    pub name: String,
    pub changes: usize,
}

/// Operazione lasciata a metà da un conflitto, per il banner del Commit: nome e file ancora in conflitto.
#[derive(Clone, PartialEq)]
pub struct PendingOperation {
    pub name: &'static str,
    pub conflicts: usize,
}

/// Ciò che il pannello mostra per un progetto: si scambia con quello del pannello quando cambia il progetto attivo.
#[derive(Default)]
pub struct ProjectState {
    root: Option<PathBuf>,
    repos: Vec<RepoTab>,
    commits: Vec<Commit>,
    graph: Vec<GraphRow>,
    changes: Vec<ChangedFile>,
    pub terminals: Vec<Terminal>,
    view: Option<u32>,
    /// Documento Scintilla con il log dei comandi git del progetto.
    console_doc: isize,
    log: Vec<CommandLog>,
}

pub struct Panel {
    pub visible: Cell<bool>,
    pub tab: Cell<Tab>,
    /// Altezza in pixel a 96 DPI.
    pub height: Cell<i32>,
    pub console: Sci,
    pub branches: List,
    pub log: List,
    pub details: List,
    pub files: List,
    pub message: HWND,
    /// Filtri del log: testo nel messaggio, autore, path.
    pub filter_text: HWND,
    pub filter_author: HWND,
    pub filter_path: HWND,
    /// Anteprima affiancata del file selezionato nella tab Commit: HEAD a sinistra, copia di lavoro a destra.
    pub preview: [Sci; 2],
    pub preview_path: RefCell<Option<String>>,
    /// Branch del log; None = tutti.
    pub reference: RefCell<Option<String>>,
    /// Con filtri su autore, percorso o testo i parent non sono contigui: il grafo non ha senso e si nasconde.
    pub graph_hidden: Cell<bool>,
    /// Il prossimo commit modifica l'ultimo (amend).
    pub amend: Cell<bool>,
    pub operation: RefCell<Option<PendingOperation>>,
    pub commits: RefCell<Vec<Commit>>,
    graph: RefCell<Vec<GraphRow>>,
    graph_width: Cell<usize>,
    pub changes: RefCell<Vec<ChangedFile>>,
    pub branch_list: RefCell<Vec<BranchEntry>>,
    pub branch_rows: RefCell<Vec<BranchRow>>,
    collapsed: RefCell<HashSet<String>>,
    collapsed_root: RefCell<Option<PathBuf>>,
    pub detail_rows: RefCell<Vec<DetailRow>>,
    pub commit_files: RefCell<Vec<CommitFile>>,
    /// Commit di cui si mostrano i dettagli.
    pub selected_commit: RefCell<Option<String>>,
    /// Repository mostrato da Git e Commit.
    pub root: RefCell<Option<PathBuf>>,
    pub repos: RefCell<Vec<RepoTab>>,
    /// La cartella aperta (None se non ce n'è), per il messaggio della tab Git vuota.
    pub has_folder: Cell<bool>,
    /// Comandi git eseguiti (anche quelli in background, mostrati solo con la casella).
    pub command_log: RefCell<Vec<CommandLog>>,
    pub show_background: Cell<bool>,
    pub terminals: RefCell<Vec<Terminal>>,
    /// Terminale mostrato nella Console; None è il log dei comandi git.
    pub view: Cell<Option<u32>>,
    pub next_terminal: Cell<u32>,
    tabs: RefCell<Vec<(RECT, Tab)>>,
    console_tabs: RefCell<Vec<(RECT, ConsoleTarget)>>,
    repo_tabs: RefCell<Vec<(RECT, PathBuf)>>,
    buttons: RefCell<Vec<(RECT, Button)>>,
}

fn rgb(hex: u32) -> u32 {
    ((hex & 0xFF) << 16) | (hex & 0xFF00) | ((hex >> 16) & 0xFF)
}

/// Data e ora locali di un timestamp Unix, come `14/11/2026 12:30`.
pub fn local_time(timestamp: i64) -> String {
    let ticks = (timestamp.max(0) as u64) * 10_000_000 + 116_444_736_000_000_000;
    let file_time = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    // SAFETY: strutture locali inizializzate; fuso orario corrente del sistema.
    unsafe {
        let (mut utc, mut local): (SYSTEMTIME, SYSTEMTIME) = (std::mem::zeroed(), std::mem::zeroed());
        if FileTimeToSystemTime(&file_time, &mut utc) == 0 || SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) == 0 {
            return String::new();
        }
        format!("{:02}/{:02}/{} {:02}:{:02}", local.wDay, local.wMonth, local.wYear, local.wHour, local.wMinute)
    }
}

/// Ora locale (ore:minuti:secondi) di un istante in millisecondi dall'epoca Unix.
fn local_clock(milliseconds: u128) -> String {
    let ticks = (milliseconds as u64) * 10_000 + 116_444_736_000_000_000;
    let file_time = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    // SAFETY: strutture locali inizializzate; fuso orario corrente del sistema.
    unsafe {
        let (mut utc, mut local): (SYSTEMTIME, SYSTEMTIME) = (std::mem::zeroed(), std::mem::zeroed());
        if FileTimeToSystemTime(&file_time, &mut utc) == 0 || SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) == 0 {
            return String::new();
        }
        format!("{:02}:{:02}:{:02}", local.wHour, local.wMinute, local.wSecond)
    }
}

/// Testo del log e tratti da colorare (inizio, lunghezza, stile), in byte.
fn log_text(log: &[CommandLog], background: bool) -> (String, Vec<(usize, usize, usize)>) {
    let mut text = String::new();
    let mut runs = Vec::new();
    let mut push = |text: &mut String, part: &str, style: usize| {
        if style != 0 && !part.is_empty() {
            runs.push((text.len(), part.len(), style));
        }
        text.push_str(part);
    };
    for entry in log.iter().filter(|entry| background || !entry.background) {
        let failed = entry.code != Some(0);
        push(&mut text, &local_clock(entry.started_at), STYLE_MUTED);
        push(&mut text, &format!("  $ {}  ", entry.command), 0);
        let code = entry.code.map_or_else(|| "—".to_string(), |code| code.to_string());
        push(&mut text, &format!("{code} · {} ms\n", entry.duration_ms), if failed { STYLE_DANGER } else { STYLE_MUTED });
        for (output, style) in [(&entry.stdout, 0), (&entry.stderr, STYLE_DANGER)] {
            for line in output.trim_end().lines().filter(|_| !output.trim().is_empty()) {
                push(&mut text, &format!("    {line}\n"), style);
            }
        }
    }
    if log.iter().all(|entry| entry.background && !background) {
        text.push_str("Nessun comando eseguito.");
    }
    (text, runs)
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// Tempo trascorso in italiano, come `Intl.RelativeTimeFormat("it", { numeric: "auto" })` della versione Tauri.
pub fn relative_time(timestamp: i64, now: i64) -> String {
    const UNITS: [(i64, &str, &str, &str); 6] = [
        (31_536_000, "anno", "anni", "l'anno scorso"),
        (2_592_000, "mese", "mesi", "il mese scorso"),
        (604_800, "settimana", "settimane", "la settimana scorsa"),
        (86_400, "giorno", "giorni", "ieri"),
        (3_600, "ora", "ore", ""),
        (60, "minuto", "minuti", ""),
    ];
    let elapsed = now - timestamp;
    for (seconds, one, many, last) in UNITS {
        if elapsed.abs() < seconds {
            continue;
        }
        let count = (elapsed as f64 / seconds as f64).round() as i64;
        if count < 0 {
            let count = -count;
            return format!("tra {count} {}", if count == 1 { one } else { many });
        }
        return match count {
            1 if !last.is_empty() => last.to_string(),
            2 if seconds == 86_400 => "l'altro ieri".to_string(),
            1 => format!("1 {one} fa"),
            _ => format!("{count} {many} fa"),
        };
    }
    "adesso".to_string()
}

/// Etichetta e tipo di un ref del log: `HEAD -> main` diventa `main`, i tag perdono `tag: `.
fn ref_label(reference: &str) -> (&str, RefKind) {
    if let Some(name) = reference.strip_prefix("HEAD -> ") {
        return (name, RefKind::Head);
    }
    if let Some(name) = reference.strip_prefix("tag: ") {
        return (name, RefKind::Tag);
    }
    (reference, if reference == "HEAD" { RefKind::Head } else { RefKind::Branch })
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum RefKind {
    Head,
    Tag,
    Branch,
}

#[derive(Default)]
struct Node {
    folders: BTreeMap<String, Node>,
    leaves: Vec<(String, usize)>,
}

fn branch_tree(entries: &[(usize, &str)]) -> Node {
    let mut root = Node::default();
    for &(index, name) in entries {
        let mut parts: Vec<&str> = name.split('/').collect();
        let leaf = parts.pop().unwrap_or(name);
        let mut node = &mut root;
        for part in parts {
            node = node.folders.entry(part.to_string()).or_default();
        }
        node.leaves.push((leaf.to_string(), index));
    }
    root
}

fn flatten(node: &Node, key: &str, depth: usize, collapsed: &HashSet<String>, rows: &mut Vec<BranchRow>) {
    for (name, child) in &node.folders {
        let child_key = format!("{key}/{name}");
        rows.push(BranchRow::Folder { key: child_key.clone(), name: name.clone(), depth });
        if !collapsed.contains(&child_key) {
            flatten(child, &child_key, depth + 1, collapsed, rows);
        }
    }
    let mut leaves = node.leaves.clone();
    leaves.sort_by_key(|(name, _)| name.to_lowercase());
    rows.extend(leaves.into_iter().map(|(name, index)| BranchRow::Branch { index, name, depth }));
}

/// Righe della lista dei branch: tutti, poi locali e remoti ad albero sui `/` del nome.
pub fn branch_rows(branches: &[BranchEntry], collapsed: &HashSet<String>) -> Vec<BranchRow> {
    let mut rows = vec![BranchRow::All];
    for (remote, title, key) in [(false, "Locali", "local"), (true, "Remoti", "remote")] {
        let entries: Vec<(usize, &str)> = branches.iter().enumerate().filter(|(_, branch)| branch.remote == remote).map(|(index, branch)| (index, branch.name.as_str())).collect();
        if entries.is_empty() {
            continue;
        }
        rows.push(BranchRow::Header(title));
        flatten(&branch_tree(&entries), key, 0, collapsed, &mut rows);
    }
    rows
}

/// Cartelle da chiudere all'inizio: tutte tranne quelle sul percorso del branch corrente.
pub fn folders_to_collapse(branches: &[BranchEntry]) -> HashSet<String> {
    let mut folders = HashSet::new();
    let mut open = HashSet::new();
    for branch in branches {
        let key = if branch.remote { "remote" } else { "local" };
        let parts: Vec<&str> = branch.name.split('/').collect();
        let mut path = key.to_string();
        for part in &parts[..parts.len().saturating_sub(1)] {
            path = format!("{path}/{part}");
            if branch.current {
                open.insert(path.clone());
            }
            folders.insert(path.clone());
        }
    }
    folders.difference(&open).cloned().collect()
}

impl Panel {
    pub fn create(parent: HWND, instance: HINSTANCE, palette: &Palette) -> Panel {
        let scintilla = wide("Scintilla");
        let edit = wide("EDIT");
        let child = |class: &[u16], style: u32| {
            // SAFETY: controllo figlio della finestra principale.
            unsafe { CreateWindowExW(0, class.as_ptr(), std::ptr::null(), style, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null()) }
        };
        let console = child(&scintilla, WS_CHILD);
        let preview = [child(&scintilla, WS_CHILD), child(&scintilla, WS_CHILD)];
        let message = child(&edit, WS_CHILD | WS_VSCROLL | WS_TABSTOP | (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32);
        let filters = ["Testo nel messaggio", "Autore", "Path (es. src/)"].map(|cue| {
            let field = child(&edit, WS_CHILD | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32);
            let cue = wide(cue);
            // SAFETY: campo appena creato; il testo vive per la chiamata.
            unsafe { SendMessageW(field, 0x1501, 1, cue.as_ptr() as isize) };
            field
        });
        let cue = wide("Messaggio di commit (Ctrl+Invio per committare)");
        // SAFETY: controlli appena creati; i testi vivono per le chiamate.
        unsafe {
            SendMessageW(message, 0x1501, 1, cue.as_ptr() as isize);
            if palette.dark {
                let theme = wide("DarkMode_Explorer");
                for control in [console, preview[0], preview[1], message] {
                    SetWindowTheme(control, theme.as_ptr(), std::ptr::null());
                }
            }
        }
        let console = Sci::new(console);
        let preview = preview.map(Sci::new);
        for pane in &preview {
            crate::side_by_side::configure(pane, palette);
        }
        {
            let sci = &console;
            sci.call_str(SCI_STYLESETFONT, STYLE_DEFAULT, "Cascadia Mono");
            sci.call(SCI_STYLESETSIZEFRACTIONAL, STYLE_DEFAULT, 975);
            sci.call(SCI_STYLESETFORE, STYLE_DEFAULT, palette.fg as isize);
            sci.call(SCI_STYLESETBACK, STYLE_DEFAULT, palette.editor as isize);
            sci.call(SCI_STYLECLEARALL, 0, 0);
            sci.call(SCI_SETMARGINWIDTHN, 0, 0);
            sci.call(SCI_SETMARGINWIDTHN, 1, 0);
            sci.call(SCI_SETCODEPAGE, SC_CP_UTF8, 0);
            sci.call(SCI_SETUNDOCOLLECTION, 0, 0);
            sci.call(SCI_SETREADONLY, 1, 0);
            sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_BACK, (palette.selection | 0xFF00_0000) as isize);
            sci.call(SCI_STYLESETFORE, STYLE_DANGER, palette.danger as isize);
            sci.call(SCI_STYLESETFORE, STYLE_MUTED, palette.muted as isize);
        }
        let list = || List::create(parent, instance, palette.editor, palette.dark);
        Panel {
            visible: Cell::new(false),
            tab: Cell::new(Tab::Console),
            height: Cell::new(200),
            console,
            branches: list(),
            log: list(),
            details: list(),
            files: list(),
            message,
            filter_text: filters[0],
            filter_author: filters[1],
            filter_path: filters[2],
            preview,
            preview_path: RefCell::new(None),
            reference: RefCell::new(None),
            graph_hidden: Cell::new(false),
            amend: Cell::new(false),
            operation: RefCell::new(None),
            commits: RefCell::new(Vec::new()),
            graph: RefCell::new(Vec::new()),
            graph_width: Cell::new(1),
            changes: RefCell::new(Vec::new()),
            branch_list: RefCell::new(Vec::new()),
            branch_rows: RefCell::new(Vec::new()),
            collapsed: RefCell::default(),
            collapsed_root: RefCell::new(None),
            detail_rows: RefCell::new(Vec::new()),
            commit_files: RefCell::new(Vec::new()),
            selected_commit: RefCell::new(None),
            root: RefCell::new(None),
            repos: RefCell::new(Vec::new()),
            has_folder: Cell::new(false),
            command_log: RefCell::new(Vec::new()),
            show_background: Cell::new(false),
            terminals: RefCell::new(Vec::new()),
            view: Cell::new(None),
            next_terminal: Cell::new(1),
            tabs: RefCell::new(Vec::new()),
            console_tabs: RefCell::new(Vec::new()),
            repo_tabs: RefCell::new(Vec::new()),
            buttons: RefCell::new(Vec::new()),
        }
    }

    /// Stato vuoto per un progetto nuovo, con il suo documento per la Console.
    pub fn new_state(&self) -> ProjectState {
        ProjectState { console_doc: self.console.call(SCI_CREATEDOCUMENT, 0, 0), ..ProjectState::default() }
    }

    pub fn swap_state(&self, state: &mut ProjectState) {
        for terminal in self.terminals.borrow().iter() {
            // SAFETY: controllo valido creato dal terminale.
            unsafe { ShowWindow(terminal.sci.hwnd, SW_HIDE) };
        }
        std::mem::swap(&mut *self.root.borrow_mut(), &mut state.root);
        std::mem::swap(&mut *self.repos.borrow_mut(), &mut state.repos);
        std::mem::swap(&mut *self.commits.borrow_mut(), &mut state.commits);
        std::mem::swap(&mut *self.graph.borrow_mut(), &mut state.graph);
        std::mem::swap(&mut *self.changes.borrow_mut(), &mut state.changes);
        std::mem::swap(&mut *self.terminals.borrow_mut(), &mut state.terminals);
        std::mem::swap(&mut *self.command_log.borrow_mut(), &mut state.log);
        state.view = self.view.replace(state.view);
        // La vista tiene un riferimento al documento mostrato: quello uscente va tenuto vivo, quello entrante passa alla vista.
        let current = self.console.call(SCI_GETDOCPOINTER, 0, 0);
        self.console.call(SCI_ADDREFDOCUMENT, 0, current);
        self.console.call(SCI_SETDOCPOINTER, 0, state.console_doc);
        self.console.call(SCI_RELEASEDOCUMENT, 0, state.console_doc);
        state.console_doc = current;
        self.console.call(SCI_SETCODEPAGE, SC_CP_UTF8, 0);
        self.console.call(SCI_SETUNDOCOLLECTION, 0, 0);
        self.console.call(SCI_SETREADONLY, 1, 0);
        self.graph_width.set(self.graph.borrow().iter().map(|row| row.width).max().unwrap_or(1).max(1));
        self.log.set_rows(self.commits.borrow().len());
        self.files.set_rows(self.changes.borrow().len());
        self.branch_list.borrow_mut().clear();
        self.branch_rows.borrow_mut().clear();
        self.branches.set_rows(0);
        self.clear_details();
        *self.preview_path.borrow_mut() = None;
        for pane in &self.preview {
            pane.call(SCI_SETREADONLY, 0, 0);
            pane.set_text(b"");
        }
        self.render_log();
    }

    /// Progetto chiuso: terminali e documento della Console.
    pub fn discard_state(&self, state: ProjectState) {
        for terminal in state.terminals {
            terminal.close();
        }
        self.console.call(SCI_RELEASEDOCUMENT, 0, state.console_doc);
    }

    pub fn attach(&'static self) {
        for list in [&self.branches, &self.log, &self.details, &self.files] {
            list.attach();
        }
    }

    pub fn owns(&self, hwnd: HWND) -> bool {
        [self.console.hwnd, self.branches.hwnd, self.log.hwnd, self.details.hwnd, self.files.hwnd, self.message, self.filter_text, self.filter_author, self.filter_path, self.preview[0].hwnd, self.preview[1].hwnd].contains(&hwnd)
    }

    pub fn is_filter(&self, hwnd: HWND) -> bool {
        [self.filter_text, self.filter_author, self.filter_path].contains(&hwnd)
    }

    /// Corpo del pannello sotto l'intestazione e, con più repository, sotto le loro tab.
    fn body(&self, rect: RECT, scale: &impl Fn(i32) -> i32) -> RECT {
        let repos = self.tab.get() != Tab::Console && self.repos.borrow().len() > 1;
        RECT { top: rect.top + scale(HEADER) + if repos { scale(REPO_ROW) } else { 0 }, ..rect }
    }

    fn commit_split(body: &RECT) -> i32 {
        body.left + ((body.right - body.left) * 45 / 100).max(320).min(body.right - body.left)
    }

    /// Posiziona i controlli della tab attiva in `rect` (pixel reali, intestazione compresa) e nasconde gli altri.
    pub fn layout(&self, rect: RECT, scale: impl Fn(i32) -> i32) {
        let visible = self.visible.get();
        let tab = self.tab.get();
        let body = self.body(rect, &scale);
        let place = |hwnd: HWND, shown: bool, area: RECT| {
            // SAFETY: controlli figli validi.
            unsafe {
                ShowWindow(hwnd, if shown { SW_SHOW } else { SW_HIDE });
                if shown {
                    MoveWindow(hwnd, area.left, area.top, (area.right - area.left).max(0), (area.bottom - area.top).max(0), 1);
                }
            }
        };
        let console = visible && tab == Tab::Console;
        let body_console = RECT { top: rect.top + scale(HEADER) + scale(CONSOLE_ROW), ..rect };
        place(self.console.hwnd, console && self.view.get().is_none(), RECT { top: body_console.top + scale(CONSOLE_ROW), ..body_console });
        for terminal in self.terminals.borrow().iter() {
            let shown = console && self.view.get() == Some(terminal.id);
            place(terminal.sci.hwnd, shown, body_console);
            if shown {
                terminal.fit();
            }
        }
        let repo = self.root.borrow().is_some();
        let git = visible && tab == Tab::Git && repo;
        let details = git && self.selected_commit.borrow().is_some();
        let center = RECT { left: body.left + scale(BRANCHES_WIDTH), right: if details { body.right - scale(DETAILS_WIDTH) } else { body.right }, ..body };
        place(self.branches.hwnd, git, RECT { right: center.left - 1, ..body });
        place(self.details.hwnd, details, RECT { left: center.right + 1, ..body });
        let field = (center.right - center.left - scale(16)) / 3 - scale(6);
        for (index, edit) in [self.filter_text, self.filter_author, self.filter_path].into_iter().enumerate() {
            let left = center.left + scale(8) + index as i32 * (field.min(scale(220)) + scale(6));
            place(edit, git, RECT { left, top: center.top + scale(4), right: left + field.min(scale(220)), bottom: center.top + scale(FILTER_ROW) - scale(4) });
        }
        place(self.log.hwnd, git, RECT { top: center.top + scale(FILTER_ROW), ..center });

        let commit = visible && tab == Tab::Commit && repo;
        let split = Self::commit_split(&body);
        let left = RECT { right: split - 1, ..body };
        let operation = self.operation.borrow().is_some();
        let actions_top = left.bottom - scale(BUTTON_ROW);
        let message_top = if operation { actions_top - scale(28) } else { actions_top - scale(MESSAGE_HEIGHT) };
        place(self.files.hwnd, commit, RECT { top: left.top + scale(LIST_HEADER), bottom: message_top - scale(4), ..left });
        place(self.message, commit && !operation, RECT { left: left.left + scale(8), top: message_top, right: left.right - scale(8), bottom: actions_top });
        let pane_top = body.top + scale(LIST_HEADER);
        let middle = (split + 1 + body.right) / 2;
        place(self.preview[0].hwnd, commit, RECT { left: split + 1, top: pane_top, right: middle - 1, ..body });
        place(self.preview[1].hwnd, commit, RECT { left: middle, top: pane_top, ..body });
    }

    unsafe fn button(dc: HDC, palette: &Palette, area: RECT, label: &str, primary: bool, enabled: bool) {
        if primary && enabled {
            fill(dc, &area, palette.accent);
            text(dc, label, &area, palette.editor, DT_CENTER);
        } else {
            frame(dc, &area, palette.border);
            text(dc, label, &area, if enabled { palette.fg } else { palette.muted }, DT_CENTER);
        }
    }

    unsafe fn checkbox(dc: HDC, palette: &Palette, area: RECT, full: bool, partial: bool) {
        frame(dc, &area, palette.muted);
        if full {
            fill(dc, &RECT { left: area.left + 3, top: area.top + 3, right: area.right - 3, bottom: area.bottom - 3 }, palette.accent);
        } else if partial {
            let middle = (area.top + area.bottom) / 2;
            fill(dc, &RECT { left: area.left + 3, top: middle - 1, right: area.right - 3, bottom: middle + 1 }, palette.accent);
        }
    }

    /// Da chiuso resta solo l'intestazione, così le tab si possono sempre cliccare.
    pub unsafe fn paint(&self, dc: HDC, rect: RECT, palette: &Palette, scale: impl Fn(i32) -> i32) {
        let visible = self.visible.get();
        let header = RECT { bottom: rect.top + scale(HEADER), ..rect };
        fill(dc, &rect, palette.panel);
        fill(dc, &RECT { bottom: rect.top + 1, ..rect }, palette.border);
        fill(dc, &RECT { top: header.bottom - 1, ..header }, palette.border);
        let mut left = rect.left + scale(4);
        let mut tabs = Vec::new();
        for (label, tab) in [("Console", Tab::Console), ("Git", Tab::Git), ("Commit", Tab::Commit)] {
            let width = text_width(dc, label) + scale(24);
            let area = RECT { left, top: header.top, right: left + width, bottom: header.bottom };
            let active = visible && tab == self.tab.get();
            if active {
                fill(dc, &RECT { top: area.bottom - scale(2), ..area }, palette.accent);
            }
            text(dc, label, &area, if active { palette.accent } else { palette.fg }, DT_CENTER);
            tabs.push((area, tab));
            left += width;
        }
        *self.tabs.borrow_mut() = tabs;
        let mut console_tabs = Vec::new();
        let mut console_buttons = Vec::new();
        if visible && self.tab.get() == Tab::Console {
            // Sotto-tab come nella versione Tauri: Comandi Git, un tab per terminale con la ×, + Terminale e la freccia delle shell.
            let row = RECT { top: header.bottom, bottom: header.bottom + scale(CONSOLE_ROW), ..rect };
            fill(dc, &RECT { top: row.bottom - 1, ..row }, palette.border);
            let mut left = row.left + scale(6);
            let mut targets: Vec<(String, ConsoleTarget, bool)> = vec![("Comandi Git".to_owned(), ConsoleTarget::Log, false)];
            targets.extend(self.terminals.borrow().iter().map(|terminal| (terminal.title.clone(), ConsoleTarget::Terminal(terminal.id), terminal.exited.get())));
            targets.push(("+ Terminale".to_owned(), ConsoleTarget::New, false));
            targets.push(("⌄".to_owned(), ConsoleTarget::Shells, false));
            for (label, target, exited) in targets {
                let close = matches!(target, ConsoleTarget::Terminal(_));
                let width = text_width(dc, &label) + scale(if close { 34 } else { 16 });
                let area = RECT { left, top: row.top + scale(2), right: left + width, bottom: row.bottom - scale(2) };
                let active = match target {
                    ConsoleTarget::Log => self.view.get().is_none(),
                    ConsoleTarget::Terminal(id) => self.view.get() == Some(id),
                    _ => false,
                };
                let color = if active { palette.accent } else if exited || matches!(target, ConsoleTarget::New | ConsoleTarget::Shells) { palette.muted } else { palette.fg };
                let label_area = RECT { left: area.left + scale(8), right: area.right - scale(if close { 26 } else { 8 }), ..area };
                text(dc, &label, &label_area, color, DT_LEFT);
                console_tabs.push((area, target));
                if let ConsoleTarget::Terminal(id) = target {
                    let cross = RECT { left: area.right - scale(22), right: area.right - scale(6), ..area };
                    text(dc, "×", &cross, palette.muted, DT_CENTER);
                    console_tabs.insert(console_tabs.len() - 1, (cross, ConsoleTarget::Close(id)));
                }
                left = area.right + scale(2);
            }
            // Barra del log dei comandi: casella "Comandi in background", quanti sono nascosti, Copia e Pulisci.
            if self.view.get().is_none() {
                let bar = RECT { top: row.bottom, bottom: row.bottom + scale(CONSOLE_ROW), ..rect };
                fill(dc, &RECT { top: bar.bottom - 1, ..bar }, palette.border);
                let size = scale(CHECKBOX);
                let middle = (bar.top + bar.bottom) / 2;
                let check = RECT { left: bar.left + scale(8), top: middle - size / 2, right: bar.left + scale(8) + size, bottom: middle + size / 2 };
                Self::checkbox(dc, palette, check, self.show_background.get(), false);
                let label = "Comandi in background";
                let label_area = RECT { left: check.right + scale(6), right: check.right + scale(6) + text_width(dc, label), ..bar };
                text(dc, label, &label_area, palette.muted, DT_LEFT);
                console_buttons.push((RECT { left: check.left, right: label_area.right, ..bar }, Button::Background));
                let hidden = if self.show_background.get() { 0 } else { self.command_log.borrow().iter().filter(|entry| entry.background).count() };
                if hidden > 0 {
                    text(dc, &format!("({hidden} nascosti)"), &RECT { left: label_area.right + scale(8), ..bar }, palette.muted, DT_LEFT);
                }
                let mut right = bar.right - scale(8);
                for (label, button) in [("Pulisci", Button::ClearLog), ("Copia", Button::CopyLog)] {
                    let area = RECT { left: right - text_width(dc, label) - scale(16), top: bar.top + scale(2), right, bottom: bar.bottom - scale(2) };
                    text(dc, label, &area, palette.fg, DT_CENTER);
                    console_buttons.push((area, button));
                    right = area.left - scale(4);
                }
            }
        }
        *self.console_tabs.borrow_mut() = console_tabs;
        let mut repo_tabs = Vec::new();
        let mut buttons = Vec::new();
        if !visible || self.tab.get() == Tab::Console {
            *self.repo_tabs.borrow_mut() = repo_tabs;
            *self.buttons.borrow_mut() = console_buttons;
            return;
        }

        // Una tab per repository, con il numero di file cambiati.
        let repos = self.repos.borrow();
        if repos.len() > 1 {
            let row = RECT { top: header.bottom, bottom: header.bottom + scale(REPO_ROW), ..rect };
            fill(dc, &RECT { top: row.bottom - 1, ..row }, palette.border);
            let mut left = row.left + scale(6);
            let root = self.root.borrow();
            for repo in repos.iter() {
                let count = if repo.changes > 0 { repo.changes.to_string() } else { String::new() };
                let badge = if count.is_empty() { 0 } else { text_width(dc, &count) + scale(12) };
                let width = text_width(dc, &repo.name) + scale(20) + badge;
                let area = RECT { left, top: row.top + scale(4), right: left + width, bottom: row.bottom - scale(4) };
                let active = root.as_deref() == Some(repo.root.as_path());
                if active {
                    fill(dc, &area, palette.selection);
                }
                text(dc, &repo.name, &RECT { left: area.left + scale(10), right: area.right - badge, ..area }, if active { palette.fg } else { palette.muted }, DT_LEFT | DT_END_ELLIPSIS);
                if badge > 0 {
                    let pill = RECT { left: area.right - badge, top: area.top + scale(3), right: area.right - scale(6), bottom: area.bottom - scale(3) };
                    fill(dc, &pill, palette.border);
                    text(dc, &count, &pill, palette.fg, DT_CENTER);
                }
                repo_tabs.push((area, repo.root.clone()));
                left = area.right + scale(4);
            }
        }
        *self.repo_tabs.borrow_mut() = repo_tabs;
        let body = self.body(rect, &scale);

        // Nessun repository: come nella versione Tauri, git init e clona.
        if self.root.borrow().is_none() {
            let line = |top: i32| RECT { left: body.left + scale(16), top: body.top + scale(top), right: body.right - scale(16), bottom: body.top + scale(top + 24) };
            let message = if self.has_folder.get() { "La cartella aperta non è un repository Git." } else { "Nessuna cartella aperta." };
            text(dc, message, &line(16), palette.muted, DT_LEFT);
            let mut left = body.left + scale(16);
            let mut entries = vec![("Clona repository…", Button::Clone)];
            if self.has_folder.get() {
                entries.insert(0, ("Inizializza repository (git init)", Button::Init));
            }
            for (label, button) in entries {
                let area = RECT { left, top: body.top + scale(48), right: left + text_width(dc, label) + scale(24), bottom: body.top + scale(76) };
                Self::button(dc, palette, area, label, button == Button::Init, true);
                buttons.push((area, button));
                left = area.right + scale(8);
            }
            *self.buttons.borrow_mut() = buttons;
            return;
        }

        if self.tab.get() == Tab::Git {
            let details = self.selected_commit.borrow().is_some();
            let center = RECT { left: body.left + scale(BRANCHES_WIDTH), right: if details { body.right - scale(DETAILS_WIDTH) } else { body.right }, ..body };
            fill(dc, &RECT { left: center.left - 1, right: center.left, ..body }, palette.border);
            if details {
                fill(dc, &RECT { left: center.right, right: center.right + 1, ..body }, palette.border);
            }
            if self.graph_hidden.get() {
                let field = (center.right - center.left - scale(16)) / 3 - scale(6);
                let note_left = center.left + scale(8) + 3 * (field.min(scale(220)) + scale(6));
                text(dc, "grafo nascosto con i filtri", &RECT { left: note_left, top: center.top, right: center.right - scale(8), bottom: center.top + scale(FILTER_ROW) }, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
            }
        } else {
            let split = Self::commit_split(&body);
            fill(dc, &RECT { left: split - 1, right: split, ..body }, palette.border);
            let changes = self.changes.borrow();
            let staged = changes.iter().filter(|file| file.staged).count();
            let all = !changes.is_empty() && changes.iter().all(|file| file.staged && !file.unstaged);
            let header_row = RECT { right: split - 1, bottom: body.top + scale(LIST_HEADER), ..body };
            let size = scale(CHECKBOX);
            let middle = (header_row.top + header_row.bottom) / 2;
            let check = RECT { left: header_row.left + scale(8), top: middle - size / 2, right: header_row.left + scale(8) + size, bottom: middle + size / 2 };
            Self::checkbox(dc, palette, check, all, staged > 0);
            let label = format!("Modifiche ({}) · {staged} in stage", changes.len());
            text(dc, &label, &RECT { left: check.right + scale(8), ..header_row }, palette.fg, DT_LEFT | DT_END_ELLIPSIS);
            buttons.push((RECT { left: check.left, right: check.right + scale(8) + text_width(dc, &label), ..header_row }, Button::StageAll));

            let row = RECT { top: body.bottom - scale(BUTTON_ROW), right: split - 1, ..body };
            let mut left = row.left + scale(8);
            let message = crate::find::window_text(self.message);
            let operation = self.operation.borrow().clone();
            let mut entries: Vec<(String, Button, bool, bool)> = Vec::new();
            if let Some(operation) = &operation {
                let banner = RECT { top: row.top - scale(28), bottom: row.top, ..row };
                let detail = if operation.conflicts > 0 { format!("{} file in conflitto: risolvili e mettili in stage", operation.conflicts) } else { "conflitti risolti".into() };
                text(dc, &format!("{} in corso · {detail}", operation.name), &RECT { left: banner.left + scale(8), ..banner }, palette.danger, DT_LEFT | DT_END_ELLIPSIS);
                entries.push((format!("Continua {}", operation.name.to_lowercase()), Button::Continue, true, operation.conflicts == 0));
                entries.push((format!("Annulla {}", operation.name.to_lowercase()), Button::Abort, false, true));
            } else {
                let can_commit = !message.trim().is_empty() && (staged > 0 || self.amend.get());
                entries.push(((if self.amend.get() { "Amend" } else { "Commit" }).into(), Button::Commit, true, can_commit));
                if !self.amend.get() {
                    entries.push(("Commit and Push".into(), Button::CommitAndPush, false, can_commit));
                }
            }
            for (label, button, primary, enabled) in entries {
                let width = text_width(dc, &label) + scale(24);
                let area = RECT { left, top: row.top + scale(6), right: left + width, bottom: row.bottom - scale(6) };
                Self::button(dc, palette, area, &label, primary, enabled);
                if enabled {
                    buttons.push((area, button));
                }
                left += width + scale(8);
            }
            if operation.is_none() {
                let check = RECT { left: left + scale(4), top: (row.top + row.bottom) / 2 - size / 2, right: left + scale(4) + size, bottom: (row.top + row.bottom) / 2 + size / 2 };
                Self::checkbox(dc, palette, check, self.amend.get(), false);
                let label_area = RECT { left: check.right + scale(6), right: check.right + scale(6) + text_width(dc, "Amend"), ..row };
                text(dc, "Amend", &label_area, palette.fg, DT_LEFT);
                buttons.push((RECT { left: check.left, right: label_area.right, ..row }, Button::Amend));
            }

            let preview_header = RECT { left: split + scale(8), bottom: body.top + scale(LIST_HEADER), right: body.right - scale(8), ..body };
            match self.preview_path.borrow().as_deref() {
                Some(path) => {
                    let label = "Apri nell'editor";
                    let area = RECT { left: preview_header.right - text_width(dc, label) - scale(20), top: preview_header.top + scale(3), bottom: preview_header.bottom - scale(3), ..preview_header };
                    Self::button(dc, palette, area, label, false, true);
                    buttons.push((area, Button::OpenInEditor));
                    text(dc, path, &RECT { right: area.left - scale(8), ..preview_header }, palette.fg, DT_LEFT | DT_PATH_ELLIPSIS);
                }
                None => text(dc, "Seleziona un file per vedere le modifiche.", &preview_header, palette.muted, DT_LEFT),
            }
        }
        *self.buttons.borrow_mut() = buttons;
    }

    fn hit<T: Copy>(items: &[(RECT, T)], x: i32, y: i32) -> Option<T> {
        items.iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, item)| *item)
    }

    pub fn tab_at(&self, x: i32, y: i32) -> Option<Tab> {
        Self::hit(&self.tabs.borrow(), x, y)
    }

    /// La × viene prima del suo tab nella lista, così vince sul clic.
    pub fn console_target_at(&self, x: i32, y: i32) -> Option<ConsoleTarget> {
        Self::hit(&self.console_tabs.borrow(), x, y)
    }

    pub fn button_at(&self, x: i32, y: i32) -> Option<Button> {
        Self::hit(&self.buttons.borrow(), x, y)
    }

    pub fn repo_at(&self, x: i32, y: i32) -> Option<PathBuf> {
        self.repo_tabs.borrow().iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, root)| root.clone())
    }

    /// Il controllo che riceve il focus nella Console: il terminale scelto o il log.
    pub fn console_focus(&self) -> HWND {
        let view = self.view.get();
        self.terminals.borrow().iter().find(|terminal| Some(terminal.id) == view).map_or(self.console.hwnd, |terminal| terminal.sci.hwnd)
    }

    pub fn terminal_at(&self, hwnd: HWND) -> Option<usize> {
        self.terminals.borrow().iter().position(|terminal| terminal.sci.hwnd == hwnd)
    }

    pub fn set_commits(&self, commits: Vec<Commit>) {
        let graph = layout_graph(commits.iter().map(|commit| (commit.hash.as_str(), commit.parents.as_slice())));
        self.graph_width.set(graph.iter().map(|row| row.width).max().unwrap_or(1).max(1));
        *self.graph.borrow_mut() = graph;
        let count = commits.len();
        *self.commits.borrow_mut() = commits;
        self.log.set_rows(count);
    }

    /// File cambiati del repository, con lo stato dell'index; operazione in corso e conflitti per il banner.
    pub fn set_changes(&self, root: &Path, status: &RepoStatus) {
        let files: Vec<ChangedFile> = status
            .files
            .iter()
            .filter(|entry| entry.status != FileStatus::Ignored)
            .map(|entry| {
                let relative = Path::new(&entry.path).strip_prefix(root).map_or_else(|_| entry.path.clone(), |path| path.to_string_lossy().into_owned());
                ChangedFile { path: entry.path.clone(), relative, status: entry.status, staged: entry.staged, unstaged: entry.unstaged }
            })
            .collect();
        let conflicts = files.iter().filter(|file| file.status == FileStatus::Conflict).count();
        *self.operation.borrow_mut() = status.operation.as_ref().map(|operation| PendingOperation {
            name: match operation {
                steroitor_core::git::parse::Operation::Merge => "Merge",
                steroitor_core::git::parse::Operation::Rebase => "Rebase",
                steroitor_core::git::parse::Operation::CherryPick => "Cherry-pick",
                steroitor_core::git::parse::Operation::Revert => "Revert",
            },
            conflicts,
        });
        let count = files.len();
        *self.changes.borrow_mut() = files;
        self.files.set_rows(count);
    }

    /// Branch del repository; le cartelle si chiudono tutte (tranne quella del branch corrente) quando cambia repository.
    pub fn set_branches(&self, root: &Path, branches: Vec<BranchEntry>) {
        if self.collapsed_root.borrow().as_deref() != Some(root) {
            *self.collapsed.borrow_mut() = folders_to_collapse(&branches);
            *self.collapsed_root.borrow_mut() = Some(root.to_path_buf());
        }
        *self.branch_list.borrow_mut() = branches;
        self.rebuild_branch_rows();
    }

    fn rebuild_branch_rows(&self) {
        let rows = branch_rows(&self.branch_list.borrow(), &self.collapsed.borrow());
        let count = rows.len();
        *self.branch_rows.borrow_mut() = rows;
        self.branches.set_rows(count);
    }

    pub fn toggle_folder(&self, key: &str) {
        {
            let mut collapsed = self.collapsed.borrow_mut();
            if !collapsed.remove(key) {
                collapsed.insert(key.to_string());
            }
        }
        self.rebuild_branch_rows();
    }

    pub fn clear_details(&self) {
        *self.selected_commit.borrow_mut() = None;
        self.detail_rows.borrow_mut().clear();
        self.commit_files.borrow_mut().clear();
        self.details.set_rows(0);
    }

    /// Dettagli del commit: messaggio, hash, autore, data, parent; i file arrivano dopo con `set_commit_files`.
    pub fn show_details(&self, commit: &Commit) {
        let mut rows = vec![DetailRow::Subject(commit.subject.clone())];
        rows.extend(commit.body.trim_end().lines().map(|line| DetailRow::Body(line.to_string())));
        rows.push(DetailRow::Field("Commit", commit.hash.clone()));
        rows.push(DetailRow::Field("Autore", format!("{} <{}>", commit.author, commit.email)));
        rows.push(DetailRow::Field("Data", local_time(commit.timestamp)));
        if !commit.parents.is_empty() {
            let parents = commit.parents.iter().map(|parent| parent.get(..7).unwrap_or(parent)).collect::<Vec<_>>().join(", ");
            rows.push(DetailRow::Field(if commit.parents.len() > 1 { "Parent (merge)" } else { "Parent" }, parents));
        }
        rows.push(DetailRow::Files("File cambiati".into()));
        let count = rows.len();
        *self.detail_rows.borrow_mut() = rows;
        self.commit_files.borrow_mut().clear();
        *self.selected_commit.borrow_mut() = Some(commit.hash.clone());
        self.details.set_rows(count);
    }

    pub fn set_commit_files(&self, root: &Path, hash: &str, files: Vec<steroitor_core::git::changes::ChangedFile>) {
        if self.selected_commit.borrow().as_deref() != Some(hash) {
            return;
        }
        let files: Vec<CommitFile> = files
            .into_iter()
            .map(|file| CommitFile { relative: Path::new(&file.path).strip_prefix(root).map_or_else(|_| file.path.clone(), |path| path.to_string_lossy().into_owned()), status: file.status })
            .collect();
        let mut rows = self.detail_rows.borrow_mut();
        rows.retain(|row| !matches!(row, DetailRow::File(_)));
        if let Some(DetailRow::Files(title)) = rows.last_mut() {
            *title = if files.len() == 1 { "1 file cambiato".into() } else { format!("{} file cambiati", files.len()) };
        }
        rows.extend((0..files.len()).map(DetailRow::File));
        let count = rows.len();
        drop(rows);
        *self.commit_files.borrow_mut() = files;
        self.details.set_rows(count);
    }

    /// Anteprima affiancata del file selezionato (prima e dopo), o nulla con `path` None.
    pub fn set_preview(&self, path: Option<String>, before: &str, after: &str, palette: &Palette, font_fraction: isize) {
        let file = PathBuf::from(path.clone().unwrap_or_default());
        *self.preview_path.borrow_mut() = path;
        let _ = crate::side_by_side::show([&self.preview[0], &self.preview[1]], before, after, &file, palette, font_fraction);
    }

    pub fn preview_pane(&self, hwnd: HWND) -> bool {
        self.preview.iter().any(|pane| pane.hwnd == hwnd)
    }

    /// true se il punto (coordinate schermo) cade sulla casella di spunta della lista dei file.
    /// Clic sinistro sulla colonna delle caselle; la selezione da tastiera o dal clic destro non conta.
    pub fn on_checkbox(&self, scale: impl Fn(i32) -> i32) -> bool {
        // SAFETY: stato del tasto al momento del messaggio corrente.
        if unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState(windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_LBUTTON as i32) } >= 0 {
            return false;
        }
        let mut point = POINT { x: 0, y: 0 };
        // SAFETY: conversione della posizione del cursore nelle coordinate della lista.
        unsafe {
            GetCursorPos(&mut point);
            ScreenToClient(self.files.hwnd, &mut point);
        }
        point.x < scale(8 + CHECKBOX + 6)
    }

    pub fn append_console(&self, entries: &[CommandLog]) {
        if entries.is_empty() {
            return;
        }
        {
            let mut log = self.command_log.borrow_mut();
            log.extend(entries.iter().cloned());
            let excess = log.len().saturating_sub(LOG_LIMIT);
            log.drain(..excess);
        }
        self.render_log();
    }

    /// Testo del log come nella versione Tauri: ora, `$ comando`, codice · durata; sotto stdout e stderr (in rosso).
    pub fn render_log(&self) {
        let (text, runs) = log_text(&self.command_log.borrow(), self.show_background.get());
        self.console.call(SCI_SETREADONLY, 0, 0);
        self.console.set_text(text.as_bytes());
        for (start, length, style) in runs {
            self.console.call(SCI_STARTSTYLING, start, 0);
            self.console.call(SCI_SETSTYLING, length, style as isize);
        }
        self.console.call(SCI_SETREADONLY, 1, 0);
        self.console.call(SCI_DOCUMENTEND, 0, 0);
    }

    pub fn log_text(&self) -> String {
        log_text(&self.command_log.borrow(), self.show_background.get()).0
    }

    unsafe fn row_start(row: &RowPaint, palette: &Palette, font: HFONT) {
        SelectObject(row.dc, font);
        SetBkMode(row.dc, TRANSPARENT as i32);
        if row.selected {
            fill(row.dc, &row.rect, if row.focused { palette.selection } else { palette.active_line });
        }
    }

    pub unsafe fn paint_branch_row(&self, row: &RowPaint, palette: &Palette, font: HFONT, scale: impl Fn(i32) -> i32) {
        let rows = self.branch_rows.borrow();
        let Some(entry) = rows.get(row.index) else { return };
        let dc = row.dc;
        Self::row_start(row, palette, font);
        let reference = self.reference.borrow();
        let rect = row.rect;
        let indent = |depth: usize| rect.left + scale(8) + depth as i32 * scale(12);
        match entry {
            BranchRow::All => {
                if reference.is_none() && !row.selected {
                    fill(dc, &rect, palette.active_line);
                }
                text(dc, "Tutti i branch", &RECT { left: rect.left + scale(8), ..rect }, if reference.is_none() { palette.accent } else { palette.fg }, DT_LEFT);
            }
            BranchRow::Header(title) => text(dc, title, &RECT { left: rect.left + scale(8), ..rect }, palette.muted, DT_LEFT),
            BranchRow::Folder { key, name, depth } => {
                let open = !self.collapsed.borrow().contains(key);
                text(dc, &format!("{} {name}", if open { "▾" } else { "▸" }), &RECT { left: indent(*depth), ..rect }, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
            }
            BranchRow::Branch { index, name, depth } => {
                let branches = self.branch_list.borrow();
                let current = branches.get(*index).is_some_and(|branch| branch.current);
                let full = branches.get(*index).map(|branch| branch.name.as_str());
                if reference.as_deref() == full && full.is_some() && !row.selected {
                    fill(dc, &rect, palette.active_line);
                }
                text(dc, name, &RECT { left: indent(*depth) + scale(12), ..rect }, if current { palette.accent } else { palette.fg }, DT_LEFT | DT_END_ELLIPSIS);
            }
        }
    }

    pub unsafe fn paint_log_row(&self, row: &RowPaint, palette: &Palette, font: HFONT, scale: impl Fn(i32) -> i32) {
        let commits = self.commits.borrow();
        let graph = self.graph.borrow();
        let (Some(commit), Some(layout)) = (commits.get(row.index), graph.get(row.index)) else { return };
        let dc = row.dc;
        Self::row_start(row, palette, font);
        let rect = row.rect;
        let lane = scale(LANE);
        let middle = (rect.top + rect.bottom) / 2;
        let hidden = self.graph_hidden.get();
        let mut left = rect.left + scale(6);
        if !hidden {
            let x = |lane_index: usize| rect.left + scale(6) + lane_index as i32 * lane + lane / 2;
            for segment in &layout.segments {
                let pen = CreatePen(PS_SOLID, scale(2).max(1), rgb(LANE_COLORS[segment.color % LANE_COLORS.len()]));
                let previous = SelectObject(dc, pen);
                let (from_y, to_y) = match segment.half {
                    Half::Top => (rect.top, middle),
                    Half::Bottom => (middle, rect.bottom),
                };
                MoveToEx(dc, x(segment.from), from_y, std::ptr::null_mut());
                LineTo(dc, x(segment.to), to_y);
                SelectObject(dc, previous);
                DeleteObject(pen);
            }
            let color = rgb(LANE_COLORS[layout.color % LANE_COLORS.len()]);
            let brush = CreateSolidBrush(color);
            let pen = CreatePen(PS_SOLID, 1, color);
            let (old_brush, old_pen) = (SelectObject(dc, brush), SelectObject(dc, pen));
            let radius = if commit.parents.len() > 1 { scale(4) } else { scale(4) * 7 / 8 };
            let center = x(layout.lane);
            Ellipse(dc, center - radius, middle - radius, center + radius + 1, middle + radius + 1);
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
            DeleteObject(brush);
            DeleteObject(pen);
            left += self.graph_width.get() as i32 * lane;
        }
        // Colonne come nella versione Tauri: hash, messaggio con i ref, autore, data relativa.
        let date = RECT { left: rect.right - scale(110), right: rect.right - scale(8), ..rect };
        let author = RECT { left: date.left - scale(146), right: date.left - scale(6), ..rect };
        text(dc, &relative_time(commit.timestamp, now()), &date, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
        text(dc, &commit.author, &author, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
        text(dc, &commit.short_hash, &RECT { left, right: left + scale(64), ..rect }, palette.muted, DT_LEFT);
        left += scale(64);
        let right = author.left - scale(8);
        for reference in &commit.refs {
            let (label, kind) = ref_label(reference);
            let color = match kind {
                RefKind::Head => palette.accent,
                RefKind::Tag => palette.git_added,
                RefKind::Branch => palette.muted,
            };
            let width = (text_width(dc, label) + scale(10)).min((right - left).max(0));
            if width <= scale(10) {
                break;
            }
            let badge = RECT { left, top: rect.top + scale(3), right: left + width, bottom: rect.bottom - scale(3) };
            frame(dc, &badge, color);
            text(dc, label, &badge, color, DT_CENTER | DT_END_ELLIPSIS);
            left = badge.right + scale(4);
        }
        text(dc, &commit.subject, &RECT { left, right, ..rect }, palette.fg, DT_LEFT | DT_END_ELLIPSIS);
    }

    pub unsafe fn paint_detail_row(&self, row: &RowPaint, palette: &Palette, font: HFONT, scale: impl Fn(i32) -> i32) {
        let rows = self.detail_rows.borrow();
        let Some(entry) = rows.get(row.index) else { return };
        let dc = row.dc;
        Self::row_start(row, palette, font);
        let rect = RECT { left: row.rect.left + scale(10), right: row.rect.right - scale(8), ..row.rect };
        match entry {
            DetailRow::Subject(subject) => text(dc, subject, &rect, palette.fg, DT_LEFT | DT_END_ELLIPSIS),
            DetailRow::Body(line) => text(dc, line, &rect, palette.muted, DT_LEFT | DT_END_ELLIPSIS),
            DetailRow::Field(label, value) => {
                text(dc, label, &RECT { right: rect.left + scale(90), ..rect }, palette.muted, DT_LEFT);
                text(dc, value, &RECT { left: rect.left + scale(94), ..rect }, palette.fg, DT_LEFT | DT_END_ELLIPSIS);
            }
            DetailRow::Files(title) => text(dc, title, &rect, palette.accent, DT_LEFT),
            DetailRow::File(index) => {
                let files = self.commit_files.borrow();
                let Some(file) = files.get(*index) else { return };
                let color = match file.status.as_str() {
                    "A" => palette.git_added,
                    "D" => palette.git_ignored,
                    _ => palette.git_modified,
                };
                text(dc, &file.status, &RECT { right: rect.left + scale(14), ..rect }, color, DT_LEFT);
                let name = Path::new(&file.relative).file_name().map_or_else(|| file.relative.clone(), |name| name.to_string_lossy().into_owned());
                let name_area = RECT { left: rect.left + scale(16), right: (rect.left + scale(16) + text_width(dc, &name)).min(rect.right), ..rect };
                text(dc, &name, &name_area, color, DT_LEFT | DT_END_ELLIPSIS);
                let dir = Path::new(&file.relative).parent().map(|dir| dir.display().to_string()).unwrap_or_default();
                text(dc, &dir, &RECT { left: name_area.right + scale(8), ..rect }, palette.muted, DT_LEFT | DT_PATH_ELLIPSIS);
            }
        }
    }

    pub unsafe fn paint_file_row(&self, row: &RowPaint, palette: &Palette, font: HFONT, scale: impl Fn(i32) -> i32) {
        let changes = self.changes.borrow();
        let Some(file) = changes.get(row.index) else { return };
        let dc = row.dc;
        Self::row_start(row, palette, font);
        let rect = row.rect;
        let size = scale(CHECKBOX);
        let middle = (rect.top + rect.bottom) / 2;
        let checkbox = RECT { left: rect.left + scale(8), top: middle - size / 2, right: rect.left + scale(8) + size, bottom: middle + size / 2 };
        Self::checkbox(dc, palette, checkbox, file.staged && !file.unstaged, file.staged && file.unstaged);
        let (letter, color) = match file.status {
            FileStatus::Added => ("A", palette.git_added),
            FileStatus::Modified => ("M", palette.git_modified),
            FileStatus::Deleted => ("D", palette.git_ignored),
            FileStatus::Untracked => ("U", palette.git_untracked),
            FileStatus::Conflict => ("C", palette.danger),
            FileStatus::Ignored => ("I", palette.git_ignored),
        };
        let left = checkbox.right + scale(8);
        text(dc, letter, &RECT { left, right: left + scale(16), ..rect }, color, DT_LEFT);
        let name = Path::new(&file.relative).file_name().map_or_else(|| file.relative.clone(), |name| name.to_string_lossy().into_owned());
        let name_area = RECT { left: left + scale(20), right: (left + scale(20) + text_width(dc, &name)).min(rect.right - scale(8)), ..rect };
        text(dc, &name, &name_area, color, DT_LEFT | DT_END_ELLIPSIS);
        let dir = Path::new(&file.relative).parent().map(|dir| dir.display().to_string()).unwrap_or_default();
        text(dc, &dir, &RECT { left: name_area.right + scale(8), right: rect.right - scale(8), ..rect }, palette.muted, DT_LEFT | DT_PATH_ELLIPSIS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_local_dates() {
        let formatted = local_time(1_700_000_000);
        assert_eq!(formatted.len(), "14/11/2023 22:13".len());
        assert!(formatted.contains("/11/2023"));
    }

    #[test]
    fn formats_relative_times_like_the_tauri_version() {
        let now = 1_700_000_000;
        assert_eq!(relative_time(now - 20, now), "adesso");
        assert_eq!(relative_time(now - 120, now), "2 minuti fa");
        assert_eq!(relative_time(now - 3_600, now), "1 ora fa");
        assert_eq!(relative_time(now - 86_400, now), "ieri");
        assert_eq!(relative_time(now - 2 * 86_400, now), "l'altro ieri");
        assert_eq!(relative_time(now - 5 * 86_400, now), "5 giorni fa");
        assert_eq!(relative_time(now - 40_000_000, now), "l'anno scorso");
    }

    #[test]
    fn writes_the_command_log_like_the_tauri_version() {
        let entry = |command: &str, code: Option<i32>, background: bool, stderr: &str| CommandLog { id: 0, command: command.into(), cwd: String::new(), code, duration_ms: 12, started_at: 0, stdout: "ok\n".into(), stderr: stderr.into(), background };
        let log = [entry("git status", Some(0), true, ""), entry("git push", Some(1), false, "rifiutato")];
        let (text, runs) = log_text(&log, false);
        assert!(!text.contains("git status") && text.contains("  $ git push  1 · 12 ms\n    ok\n    rifiutato\n"));
        assert!(runs.iter().any(|&(start, length, style)| style == STYLE_DANGER && &text[start..start + length] == "    rifiutato\n"));
        assert!(log_text(&log, true).0.contains("git status"));
        assert_eq!(log_text(&log[..1], false).0, "Nessun comando eseguito.");
    }

    #[test]
    fn labels_refs() {
        assert_eq!(ref_label("HEAD -> main"), ("main", RefKind::Head));
        assert_eq!(ref_label("tag: v1.0"), ("v1.0", RefKind::Tag));
        assert_eq!(ref_label("origin/main"), ("origin/main", RefKind::Branch));
    }

    #[test]
    fn builds_the_branch_tree() {
        let branch = |name: &str, remote: bool, current: bool| BranchEntry { name: name.into(), remote, current };
        let branches = vec![branch("main", false, true), branch("feature/b", false, false), branch("feature/a", false, false), branch("origin/main", true, false)];
        let collapsed = folders_to_collapse(&branches);
        assert!(collapsed.contains("local/feature") && collapsed.contains("remote/origin"));
        let rows = branch_rows(&branches, &HashSet::new());
        let names: Vec<String> = rows
            .iter()
            .map(|row| match row {
                BranchRow::All => "*".into(),
                BranchRow::Header(title) => title.to_string(),
                BranchRow::Folder { name, depth, .. } => format!("{depth}/{name}/"),
                BranchRow::Branch { name, depth, .. } => format!("{depth}/{name}"),
            })
            .collect();
        assert_eq!(names, ["*", "Locali", "0/feature/", "1/a", "1/b", "0/main", "Remoti", "0/origin/", "1/main"]);
        assert_eq!(branch_rows(&branches, &collapsed).len(), 6);
    }
}

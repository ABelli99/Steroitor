//! Pannello inferiore a tab: Console (comandi git eseguiti), Git (log con grafo), Commit (file da committare e messaggio).
use std::cell::{Cell, RefCell};
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
const FILTER_ROW: i32 = 32;
const SEARCH_WIDTH: i32 = 240;
const MESSAGE_HEIGHT: i32 = 90;
const BUTTON_ROW: i32 = 40;
const LANE: i32 = 14;
const CHECKBOX: i32 = 14;
const SCI_SETREADONLY: u32 = 2171;
const SCI_SETUNDOCOLLECTION: u32 = 2012;
const SCI_DOCUMENTEND: u32 = 2318;
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
}

/// Filtri del log cliccabili nella riga sopra il log; il testo si cerca nel campo accanto.
#[derive(Clone, Copy, PartialEq)]
pub enum Chip {
    Branch,
    Author,
    Path,
}

#[derive(Default, Clone)]
pub struct LogFilters {
    pub branch: Option<String>,
    pub author: Option<String>,
    pub path: Option<String>,
}

/// Sotto-tab della Console: il log dei comandi git, un terminale (o la sua ×), il pulsante per aprirne uno.
#[derive(Clone, Copy, PartialEq)]
pub enum ConsoleTarget {
    Log,
    Terminal(u32),
    Close(u32),
    New,
}

pub struct ChangedFile {
    pub path: String,
    pub relative: String,
    pub status: FileStatus,
    pub staged: bool,
    pub checked: bool,
}

/// Ciò che il pannello mostra per un progetto: si scambia con quello del pannello quando cambia il progetto attivo.
#[derive(Default)]
pub struct ProjectState {
    root: Option<PathBuf>,
    repo_count: usize,
    commits: Vec<Commit>,
    graph: Vec<GraphRow>,
    changes: Vec<ChangedFile>,
    pub terminals: Vec<Terminal>,
    view: Option<u32>,
    /// Documento Scintilla con il log dei comandi git del progetto.
    console_doc: isize,
}

pub struct Panel {
    pub visible: Cell<bool>,
    pub tab: Cell<Tab>,
    /// Altezza in pixel a 96 DPI.
    pub height: Cell<i32>,
    pub console: Sci,
    pub log: List,
    pub files: List,
    pub message: HWND,
    /// Ricerca nel messaggio dei commit, nella riga dei filtri del log.
    pub search: HWND,
    pub filters: RefCell<LogFilters>,
    /// Con filtri su autore, percorso o testo i parent non sono contigui: il grafo non ha senso e si nasconde.
    pub graph_hidden: Cell<bool>,
    chips: RefCell<Vec<(RECT, Chip)>>,
    /// Il prossimo commit modifica l'ultimo (amend).
    pub amend: Cell<bool>,
    pub commits: RefCell<Vec<Commit>>,
    graph: RefCell<Vec<GraphRow>>,
    pub changes: RefCell<Vec<ChangedFile>>,
    /// Repository mostrato da log e commit.
    pub root: RefCell<Option<PathBuf>>,
    /// Repository della cartella: con più di uno il nome nell'intestazione apre la scelta.
    pub repo_count: Cell<usize>,
    repo_rect: Cell<RECT>,
    pub terminals: RefCell<Vec<Terminal>>,
    /// Terminale mostrato nella Console; None è il log dei comandi git.
    pub view: Cell<Option<u32>>,
    pub next_terminal: Cell<u32>,
    tabs: RefCell<Vec<(RECT, Tab)>>,
    console_tabs: RefCell<Vec<(RECT, ConsoleTarget)>>,
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

impl Panel {
    pub fn create(parent: HWND, instance: HINSTANCE, palette: &Palette) -> Panel {
        let scintilla = wide("Scintilla");
        let edit = wide("EDIT");
        // SAFETY: controlli figli della finestra principale.
        let (console, message, search) = unsafe {
            let console = CreateWindowExW(0, scintilla.as_ptr(), std::ptr::null(), WS_CHILD, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null());
            let style = WS_CHILD | WS_VSCROLL | WS_TABSTOP | (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32;
            let message = CreateWindowExW(0, edit.as_ptr(), std::ptr::null(), style, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null());
            let search = CreateWindowExW(0, edit.as_ptr(), std::ptr::null(), WS_CHILD | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null());
            let cue = wide("Cerca nei messaggi");
            SendMessageW(search, 0x1501, 1, cue.as_ptr() as isize);
            if palette.dark {
                let theme = wide("DarkMode_Explorer");
                SetWindowTheme(console, theme.as_ptr(), std::ptr::null());
                SetWindowTheme(message, theme.as_ptr(), std::ptr::null());
            }
            (console, message, search)
        };
        let console = Sci::new(console);
        console.call_str(SCI_STYLESETFONT, STYLE_DEFAULT, "Cascadia Mono");
        console.call(SCI_STYLESETSIZEFRACTIONAL, STYLE_DEFAULT, 975);
        console.call(SCI_STYLESETFORE, STYLE_DEFAULT, palette.fg as isize);
        console.call(SCI_STYLESETBACK, STYLE_DEFAULT, palette.editor as isize);
        console.call(SCI_STYLECLEARALL, 0, 0);
        console.call(SCI_SETMARGINWIDTHN, 0, 0);
        console.call(SCI_SETMARGINWIDTHN, 1, 0);
        console.call(SCI_SETCODEPAGE, SC_CP_UTF8, 0);
        console.call(SCI_SETUNDOCOLLECTION, 0, 0);
        console.call(SCI_SETREADONLY, 1, 0);
        console.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_BACK, (palette.selection | 0xFF00_0000) as isize);
        Panel {
            visible: Cell::new(false),
            tab: Cell::new(Tab::Git),
            height: Cell::new(260),
            console,
            log: List::create(parent, instance, palette.editor, palette.dark),
            files: List::create(parent, instance, palette.editor, palette.dark),
            message,
            search,
            filters: RefCell::default(),
            graph_hidden: Cell::new(false),
            chips: RefCell::new(Vec::new()),
            amend: Cell::new(false),
            commits: RefCell::new(Vec::new()),
            graph: RefCell::new(Vec::new()),
            changes: RefCell::new(Vec::new()),
            root: RefCell::new(None),
            repo_count: Cell::new(0),
            repo_rect: Cell::new(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
            terminals: RefCell::new(Vec::new()),
            view: Cell::new(None),
            next_terminal: Cell::new(1),
            tabs: RefCell::new(Vec::new()),
            console_tabs: RefCell::new(Vec::new()),
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
        state.repo_count = self.repo_count.replace(state.repo_count);
        std::mem::swap(&mut *self.commits.borrow_mut(), &mut state.commits);
        std::mem::swap(&mut *self.graph.borrow_mut(), &mut state.graph);
        std::mem::swap(&mut *self.changes.borrow_mut(), &mut state.changes);
        std::mem::swap(&mut *self.terminals.borrow_mut(), &mut state.terminals);
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
        self.log.set_rows(self.commits.borrow().len());
        self.files.set_rows(self.changes.borrow().len());
    }

    /// Progetto chiuso: terminali e documento della Console.
    pub fn discard_state(&self, state: ProjectState) {
        for terminal in state.terminals {
            terminal.close();
        }
        self.console.call(SCI_RELEASEDOCUMENT, 0, state.console_doc);
    }

    pub fn attach(&'static self) {
        self.log.attach();
        self.files.attach();
    }

    pub fn owns(&self, hwnd: HWND) -> bool {
        [self.console.hwnd, self.log.hwnd, self.files.hwnd, self.message, self.search].contains(&hwnd)
    }

    /// Posiziona i controlli della tab attiva in `rect` (pixel reali, intestazione compresa) e nasconde gli altri.
    pub fn layout(&self, rect: RECT, scale: impl Fn(i32) -> i32) {
        let visible = self.visible.get();
        let tab = self.tab.get();
        let body = RECT { top: rect.top + scale(HEADER), ..rect };
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
        place(self.console.hwnd, console && self.view.get().is_none(), body);
        for terminal in self.terminals.borrow().iter() {
            let shown = console && self.view.get() == Some(terminal.id);
            place(terminal.sci.hwnd, shown, body);
            if shown {
                terminal.fit();
            }
        }
        let git = visible && tab == Tab::Git;
        let filter_row = RECT { bottom: body.top + scale(FILTER_ROW), ..body };
        place(self.search, git, RECT { left: (filter_row.right - scale(SEARCH_WIDTH + 8)).max(filter_row.left), top: filter_row.top + scale(4), right: filter_row.right - scale(8), bottom: filter_row.bottom - scale(4) });
        place(self.log.hwnd, git, RECT { top: filter_row.bottom, ..body });
        let commit = visible && tab == Tab::Commit;
        let message_top = body.bottom - scale(BUTTON_ROW) - scale(MESSAGE_HEIGHT);
        place(self.files.hwnd, commit, RECT { bottom: message_top - scale(4), ..body });
        place(self.message, commit, RECT { left: body.left + scale(8), top: message_top, right: body.right - scale(8), bottom: body.bottom - scale(BUTTON_ROW) });
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
            text(dc, label, &area, if active { palette.fg } else { palette.muted }, DT_CENTER);
            tabs.push((area, tab));
            left += width;
        }
        let mut console_tabs = Vec::new();
        if visible && self.tab.get() == Tab::Console {
            fill(dc, &RECT { left: left + scale(6), top: header.top + scale(7), right: left + scale(7), bottom: header.bottom - scale(7) }, palette.border);
            left += scale(12);
            let mut targets = vec![("Comandi git".to_owned(), ConsoleTarget::Log)];
            targets.extend(self.terminals.borrow().iter().map(|terminal| {
                let title = if terminal.exited.get() { format!("{} (terminato)", terminal.title) } else { terminal.title.clone() };
                (title, ConsoleTarget::Terminal(terminal.id))
            }));
            targets.push(("+".to_owned(), ConsoleTarget::New));
            for (label, target) in targets {
                let close = matches!(target, ConsoleTarget::Terminal(_));
                let width = text_width(dc, &label) + scale(if close { 36 } else { 20 });
                let area = RECT { left, top: header.top, right: left + width, bottom: header.bottom };
                let active = match target {
                    ConsoleTarget::Log => self.view.get().is_none(),
                    ConsoleTarget::Terminal(id) => self.view.get() == Some(id),
                    _ => false,
                };
                if active {
                    fill(dc, &RECT { top: area.bottom - scale(2), ..area }, palette.muted);
                }
                let label_area = RECT { left: area.left + scale(10), right: area.right - scale(if close { 26 } else { 10 }), ..area };
                text(dc, &label, &label_area, if active { palette.fg } else { palette.muted }, DT_LEFT);
                console_tabs.push((area, target));
                if let ConsoleTarget::Terminal(id) = target {
                    let cross = RECT { left: area.right - scale(24), right: area.right - scale(6), ..area };
                    text(dc, "×", &cross, palette.muted, DT_CENTER);
                    console_tabs.insert(console_tabs.len() - 1, (cross, ConsoleTarget::Close(id)));
                }
                left += width;
            }
        }
        *self.console_tabs.borrow_mut() = console_tabs;
        self.repo_rect.set(RECT { left: 0, top: 0, right: 0, bottom: 0 });
        if let Some(root) = self.root.borrow().as_ref() {
            let name = root.file_name().map_or_else(|| root.display().to_string(), |name| name.to_string_lossy().into_owned());
            let label = if self.repo_count.get() > 1 { format!("⎇ {name} ▾") } else { format!("⎇ {name}") };
            let right = rect.right - scale(12);
            let width = text_width(dc, &label).min(right - left);
            let area = RECT { left: right - width, right, ..header };
            text(dc, &label, &area, palette.muted, DT_RIGHT | DT_END_ELLIPSIS);
            if self.repo_count.get() > 1 {
                self.repo_rect.set(RECT { left: area.left - scale(6), right: area.right + scale(6), ..area });
            }
        }
        *self.tabs.borrow_mut() = tabs;

        let mut chips = Vec::new();
        if visible && self.tab.get() == Tab::Git {
            let row = RECT { top: header.bottom, bottom: header.bottom + scale(FILTER_ROW), ..rect };
            let filters = self.filters.borrow();
            let short = |path: &String| Path::new(path).file_name().map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());
            let labels = [
                (format!("Branch: {} ▾", filters.branch.as_deref().unwrap_or("tutti")), Chip::Branch),
                (format!("Autore: {} ▾", filters.author.as_deref().unwrap_or("tutti")), Chip::Author),
                (format!("Percorso: {} ▾", filters.path.as_ref().map_or_else(|| "tutti".to_string(), short)), Chip::Path),
            ];
            let mut left = row.left + scale(8);
            for (label, chip) in labels {
                let width = text_width(dc, &label) + scale(20);
                let area = RECT { left, top: row.top + scale(4), right: left + width, bottom: row.bottom - scale(4) };
                frame(dc, &area, palette.border);
                text(dc, &label, &area, palette.fg, DT_CENTER);
                chips.push((area, chip));
                left += width + scale(6);
            }
        }
        *self.chips.borrow_mut() = chips;

        let mut buttons = Vec::new();
        if visible && self.tab.get() == Tab::Commit {
            let row = RECT { top: rect.bottom - scale(BUTTON_ROW), ..rect };
            let mut left = row.left + scale(8);
            for (label, button) in [("Commit", Button::Commit), ("Commit e Push", Button::CommitAndPush)] {
                let width = text_width(dc, label) + scale(24);
                let area = RECT { left, top: row.top + scale(6), right: left + width, bottom: row.bottom - scale(6) };
                if button == Button::Commit {
                    fill(dc, &area, palette.accent);
                    text(dc, label, &area, palette.editor, DT_CENTER);
                } else {
                    frame(dc, &area, palette.border);
                    text(dc, label, &area, palette.fg, DT_CENTER);
                }
                buttons.push((area, button));
                left += width + scale(8);
            }
            let size = scale(CHECKBOX);
            let middle = (row.top + row.bottom) / 2;
            let check = RECT { left: left + scale(4), top: middle - size / 2, right: left + scale(4) + size, bottom: middle + size / 2 };
            frame(dc, &check, palette.muted);
            if self.amend.get() {
                fill(dc, &RECT { left: check.left + 3, top: check.top + 3, right: check.right - 3, bottom: check.bottom - 3 }, palette.accent);
            }
            let label = "Modifica l'ultimo commit";
            let label_area = RECT { left: check.right + scale(6), right: check.right + scale(6) + text_width(dc, label), ..row };
            text(dc, label, &label_area, palette.fg, DT_LEFT);
            buttons.push((RECT { left: check.left, right: label_area.right, ..row }, Button::Amend));
            left = label_area.right + scale(16);
            let checked = self.changes.borrow().iter().filter(|file| file.checked).count();
            let hint = RECT { left, right: row.right - scale(12), ..row };
            text(dc, &format!("{checked} file selezionati · Ctrl+Invio nel messaggio per il commit"), &hint, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
        }
        *self.buttons.borrow_mut() = buttons;
    }

    pub fn on_repo(&self, x: i32, y: i32) -> bool {
        let r = self.repo_rect.get();
        x >= r.left && x < r.right && y >= r.top && y < r.bottom
    }

    pub fn chip_at(&self, x: i32, y: i32) -> Option<Chip> {
        self.chips.borrow().iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, chip)| *chip)
    }

    pub fn tab_at(&self, x: i32, y: i32) -> Option<Tab> {
        self.tabs.borrow().iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, tab)| *tab)
    }

    /// La × viene prima del suo tab nella lista, così vince sul clic.
    pub fn console_target_at(&self, x: i32, y: i32) -> Option<ConsoleTarget> {
        self.console_tabs.borrow().iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, target)| *target)
    }

    /// Il controllo che riceve il focus nella Console: il terminale scelto o il log.
    pub fn console_focus(&self) -> HWND {
        let view = self.view.get();
        self.terminals.borrow().iter().find(|terminal| Some(terminal.id) == view).map_or(self.console.hwnd, |terminal| terminal.sci.hwnd)
    }

    pub fn terminal_at(&self, hwnd: HWND) -> Option<usize> {
        self.terminals.borrow().iter().position(|terminal| terminal.sci.hwnd == hwnd)
    }

    pub fn button_at(&self, x: i32, y: i32) -> Option<Button> {
        self.buttons.borrow().iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, button)| *button)
    }

    pub fn set_commits(&self, commits: Vec<Commit>) {
        *self.graph.borrow_mut() = layout_graph(commits.iter().map(|commit| (commit.hash.as_str(), commit.parents.as_slice())));
        let count = commits.len();
        *self.commits.borrow_mut() = commits;
        self.log.set_rows(count);
    }

    /// File cambiati del repository; la spunta si conserva per i file già in lista, i nuovi sono spuntati tranne i non tracciati.
    pub fn set_changes(&self, root: &Path, status: &RepoStatus) {
        let previous: Vec<(String, bool)> = self.changes.borrow().iter().map(|file| (file.path.clone(), file.checked)).collect();
        let files: Vec<ChangedFile> = status
            .files
            .iter()
            .filter(|entry| entry.status != FileStatus::Ignored)
            .map(|entry| {
                let relative = Path::new(&entry.path).strip_prefix(root).map_or_else(|_| entry.path.clone(), |path| path.to_string_lossy().into_owned());
                let checked = previous.iter().find(|(path, _)| *path == entry.path).map_or(entry.status != FileStatus::Untracked, |(_, checked)| *checked);
                ChangedFile { path: entry.path.clone(), relative, status: entry.status, staged: entry.staged, checked }
            })
            .collect();
        let count = files.len();
        *self.changes.borrow_mut() = files;
        self.files.set_rows(count);
    }

    pub fn toggle(&self, index: usize) {
        if let Some(file) = self.changes.borrow_mut().get_mut(index) {
            file.checked = !file.checked;
        }
        self.files.redraw();
    }

    /// true se il punto (coordinate schermo) cade sulla casella di spunta della lista dei file.
    pub fn on_checkbox(&self, scale: impl Fn(i32) -> i32) -> bool {
        let mut point = POINT { x: 0, y: 0 };
        // SAFETY: conversione della posizione del cursore nelle coordinate della lista.
        unsafe {
            GetCursorPos(&mut point);
            ScreenToClient(self.files.hwnd, &mut point);
        }
        point.x < scale(8 + CHECKBOX + 6)
    }

    pub fn append_console(&self, entries: &[CommandLog]) {
        let mut text = String::new();
        for entry in entries.iter().filter(|entry| !entry.background) {
            let status = match entry.code {
                Some(0) => String::new(),
                Some(code) => format!(", uscita {code}"),
                None => ", non eseguito".into(),
            };
            text.push_str(&format!("$ {}    ({} ms{status})    {}\n", entry.command, entry.duration_ms, entry.cwd));
            for output in [&entry.stdout, &entry.stderr] {
                let output = output.trim_end();
                if !output.is_empty() {
                    text.push_str(output);
                    text.push('\n');
                }
            }
        }
        if text.is_empty() {
            return;
        }
        let bytes = text.into_bytes();
        self.console.call(SCI_SETREADONLY, 0, 0);
        self.console.call(SCI_APPENDTEXT, bytes.len(), bytes.as_ptr() as isize);
        self.console.call(SCI_SETREADONLY, 1, 0);
        self.console.call(SCI_DOCUMENTEND, 0, 0);
    }

    pub unsafe fn paint_log_row(&self, row: &RowPaint, palette: &Palette, font: HFONT, scale: impl Fn(i32) -> i32) {
        let commits = self.commits.borrow();
        let graph = self.graph.borrow();
        let (Some(commit), Some(layout)) = (commits.get(row.index), graph.get(row.index)) else { return };
        let dc = row.dc;
        SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT as i32);
        let rect = row.rect;
        if row.selected {
            fill(dc, &rect, if row.focused { palette.selection } else { palette.active_line });
        }
        let lane = scale(LANE);
        let middle = (rect.top + rect.bottom) / 2;
        let x = |lane_index: usize| rect.left + scale(6) + lane_index as i32 * lane + lane / 2;
        let hidden = self.graph_hidden.get();
        for segment in layout.segments.iter().filter(|_| !hidden) {
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
        let radius = scale(4);
        let center = if hidden { x(0) } else { x(layout.lane) };
        Ellipse(dc, center - radius, middle - radius, center + radius + 1, middle + radius + 1);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(brush);
        DeleteObject(pen);

        let mut right = rect.right - scale(8);
        for (value, width, color) in [
            (commit.short_hash.as_str(), 70, palette.muted),
            (&*local_time(commit.timestamp), 120, palette.muted),
            (commit.author.as_str(), 150, palette.muted),
        ] {
            let area = RECT { left: right - scale(width), right, ..rect };
            text(dc, value, &area, color, DT_LEFT | DT_END_ELLIPSIS);
            right -= scale(width) + scale(8);
        }
        let mut left = rect.left + scale(6) + if hidden { 1 } else { layout.width as i32 } * lane + scale(6);
        if !commit.refs.is_empty() {
            let refs = commit.refs.join(", ");
            let width = text_width(dc, &refs).min((right - left) / 2);
            let area = RECT { left, right: left + width, ..rect };
            text(dc, &refs, &area, palette.accent, DT_LEFT | DT_END_ELLIPSIS);
            left += width + scale(8);
        }
        text(dc, &commit.subject, &RECT { left, right, ..rect }, palette.fg, DT_LEFT | DT_END_ELLIPSIS);
    }

    pub unsafe fn paint_file_row(&self, row: &RowPaint, palette: &Palette, font: HFONT, scale: impl Fn(i32) -> i32) {
        let changes = self.changes.borrow();
        let Some(file) = changes.get(row.index) else { return };
        let dc = row.dc;
        SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT as i32);
        let rect = row.rect;
        if row.selected {
            fill(dc, &rect, if row.focused { palette.selection } else { palette.active_line });
        }
        let size = scale(CHECKBOX);
        let middle = (rect.top + rect.bottom) / 2;
        let checkbox = RECT { left: rect.left + scale(8), top: middle - size / 2, right: rect.left + scale(8) + size, bottom: middle + size / 2 };
        frame(dc, &checkbox, palette.muted);
        if file.checked {
            fill(dc, &RECT { left: checkbox.left + 3, top: checkbox.top + 3, right: checkbox.right - 3, bottom: checkbox.bottom - 3 }, palette.accent);
        }
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
        text(dc, &file.relative, &RECT { left: left + scale(20), right: rect.right - scale(8), ..rect }, color, DT_LEFT | DT_PATH_ELLIPSIS);
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
}

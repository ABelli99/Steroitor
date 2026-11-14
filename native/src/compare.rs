//! Finestra di confronto, una sola e riusata: diff affiancato (file rispetto a HEAD, o file di un commit con la lista
//! a sinistra) e merge a 3 vie di un file in conflitto (nostra | risultato modificabile | loro).
use std::cell::{Cell, OnceCell, RefCell};
use std::path::{Path, PathBuf};

use steroitor_core::git::changes::{ChangedFile, FileRevision};
use steroitor_core::git::history::ConflictVersions;
use steroitor_core::text::{decode, encode, TextFile};
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::{SetWindowTheme, NMHDR};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_ESCAPE, VK_F7, VK_SHIFT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::{message_box, wide};
use crate::diff::{changed_lines, parse_conflicts, resolve_all, Kind};
use crate::git::{Event, Worker};
use crate::lang;
use crate::list::{decode_event, List, ListEvent, RowPaint, WM_LIST_EVENT, WM_LIST_PAINT};
use crate::paint::{fill, frame, text, text_width};
use crate::sci::*;
use crate::side_by_side::{self, MARKERS, MARKER_CONFLICT};
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorCompare";
const HEADER: i32 = 40;
const LABELS: i32 = 22;
const FILES_WIDTH: i32 = 280;
const HISTORY_WIDTH: i32 = 320;
/// Margine attorno al dialogo, come il `calc(100% - 48px)` della versione Tauri.
const INSET: i32 = 24;
/// Il dialogo chiede alla finestra principale di aprire un file (wParam: 0), letto da `take_open_request`.
pub const WM_COMPARE_OPEN: u32 = WM_APP + 41;
const ROW_HEIGHT: i32 = 24;
const SCI_SETREADONLY: u32 = 2171;
const SCI_GETMODIFY: u32 = 2159;
const SCI_GOTOLINE: u32 = 2024;

/// Risultati dei caricamenti in background, che tornano dal worker come `Event::Compare`.
pub enum Loaded {
    Files(String, Result<Vec<ChangedFile>, String>),
    /// Generazione della richiesta (per scartare le risposte superate), testo a sinistra e a destra.
    Pair(u64, Result<(String, String), String>),
    Conflict(PathBuf, Result<(ConflictVersions, TextFile), String>),
    History(PathBuf, Result<Vec<FileRevision>, String>),
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Diff,
    Commit,
    Merge,
    History,
}

#[derive(Clone, Copy, PartialEq)]
enum Button {
    Previous,
    Next,
    AllOurs,
    AllTheirs,
    Apply,
    Cancel,
    OpenInEditor,
    OnlyChanges,
}

struct MergeFile {
    path: PathBuf,
    encoding: String,
    bom: bool,
}

struct State {
    mode: Mode,
    root: PathBuf,
    title: String,
    labels: [String; 3],
    hash: String,
    files: Vec<ChangedFile>,
    /// Diff: righe allineate dove inizia ogni modifica. Merge: righe del risultato dove inizia ogni conflitto.
    stops: Vec<usize>,
    path: PathBuf,
    merge: Option<MergeFile>,
    /// Sotto il titolo: il percorso del file (diff e history).
    subtitle: String,
    revisions: Vec<FileRevision>,
    lines: Option<(u32, u32)>,
    only_changes: bool,
    /// Tipo di ogni riga del diff mostrato, per "Solo le zone cambiate".
    kinds: Vec<Kind>,
}

pub struct Compare {
    pub hwnd: HWND,
    owner: HWND,
    files: List,
    panes: [Sci; 3],
    palette: Palette,
    font: Cell<HFONT>,
    font_fraction: Cell<isize>,
    worker: Worker,
    generation: Cell<u64>,
    state: RefCell<State>,
    buttons: RefCell<Vec<(RECT, Button)>>,
    open_request: RefCell<Option<PathBuf>>,
}

thread_local! {
    static COMPARE: OnceCell<&'static Compare> = const { OnceCell::new() };
}

fn compare() -> Option<&'static Compare> {
    COMPARE.with(|cell| cell.get().copied())
}

fn contains(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

impl Compare {
    pub fn create(owner: HWND, instance: HINSTANCE, palette: &Palette, font_fraction: isize, worker: Worker) -> &'static Compare {
        let class_name = wide(CLASS_NAME);
        let scintilla = wide("Scintilla");
        // SAFETY: registrazione della classe e creazione della finestra (nascosta) e dei suoi controlli.
        let (hwnd, panes) = unsafe {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassExW(&class);
            let title = wide("Confronto");
            let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(), WS_POPUP | WS_BORDER | WS_CLIPCHILDREN, 0, 0, 0, 0, owner, std::ptr::null_mut(), instance, std::ptr::null());
            let dark: i32 = palette.dark.into();
            DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, (&dark as *const i32).cast(), 4);
            let panes = [0; 3].map(|_| {
                let pane = CreateWindowExW(0, scintilla.as_ptr(), std::ptr::null(), WS_CHILD, 0, 0, 0, 0, hwnd, std::ptr::null_mut(), instance, std::ptr::null());
                if palette.dark {
                    let theme = wide("DarkMode_Explorer");
                    SetWindowTheme(pane, theme.as_ptr(), std::ptr::null());
                }
                Sci::new(pane)
            });
            (hwnd, panes)
        };
        let compare: &'static Compare = Box::leak(Box::new(Compare {
            hwnd,
            owner,
            files: List::create(hwnd, instance, palette.editor, palette.dark),
            panes,
            palette: *palette,
            font: Cell::new(std::ptr::null_mut()),
            font_fraction: Cell::new(font_fraction),
            worker,
            generation: Cell::new(0),
            state: RefCell::new(State {
                mode: Mode::Diff,
                root: PathBuf::new(),
                title: String::new(),
                labels: Default::default(),
                hash: String::new(),
                files: Vec::new(),
                stops: Vec::new(),
                path: PathBuf::new(),
                merge: None,
                subtitle: String::new(),
                revisions: Vec::new(),
                lines: None,
                only_changes: false,
                kinds: Vec::new(),
            }),
            buttons: RefCell::new(Vec::new()),
            open_request: RefCell::new(None),
        }));
        compare.files.attach();
        for pane in &compare.panes {
            side_by_side::configure(pane, palette);
        }
        COMPARE.with(|cell| {
            let _ = cell.set(compare);
        });
        compare
    }

    pub fn set_font_fraction(&self, font_fraction: isize) {
        self.font_fraction.set(font_fraction);
    }

    pub fn owns(&self, hwnd: HWND) -> bool {
        // SAFETY: semplice interrogazione della gerarchia delle finestre.
        hwnd == self.hwnd || unsafe { IsChild(self.hwnd, hwnd) } != 0
    }

    fn scale(&self, value: i32) -> i32 {
        // SAFETY: finestra valida.
        value * unsafe { GetDpiForWindow(self.hwnd) } as i32 / 96
    }

    fn open(&self, mode: Mode, root: &Path, title: String, labels: [&str; 3], font: HFONT) {
        {
            let mut state = self.state.borrow_mut();
            state.mode = mode;
            state.root = root.to_path_buf();
            state.title = title.clone();
            state.labels = labels.map(str::to_owned);
            state.stops.clear();
            state.merge = None;
            state.files.clear();
            state.revisions.clear();
            state.subtitle.clear();
            state.kinds.clear();
        }
        self.font.set(font);
        self.files.set_rows(0);
        self.files.set_row_height(self.scale(ROW_HEIGHT));
        for pane in &self.panes {
            self.set_text(pane, "", &lang::PLAIN);
        }
        let title = wide(&title);
        // SAFETY: finestre valide; il dialogo copre l'area della finestra principale meno il margine, come nella versione Tauri.
        unsafe {
            SetWindowTextW(self.hwnd, title.as_ptr());
            let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(self.owner, &mut client);
            let mut origin = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
            ClientToScreen(self.owner, &mut origin);
            let inset = self.scale(INSET);
            MoveWindow(self.hwnd, origin.x + inset, origin.y + inset, (client.right - 2 * inset).max(400), (client.bottom - 2 * inset).max(300), 0);
            ShowWindow(self.hwnd, SW_SHOW);
            SetForegroundWindow(self.hwnd);
        }
        self.layout();
        self.invalidate();
    }

    /// Diff del file di lavoro rispetto a HEAD.
    pub fn show_file(&self, root: &Path, path: &Path, font: HFONT) {
        self.open(Mode::Diff, root, file_name(path), ["HEAD", "Copia di lavoro", ""], font);
        {
            let mut state = self.state.borrow_mut();
            state.path = path.to_path_buf();
            state.subtitle = path.strip_prefix(root).map_or_else(|_| path.display().to_string(), |relative| relative.display().to_string());
        }
        let generation = self.next_generation();
        let (root, path) = (root.to_string_lossy().into_owned(), path.to_path_buf());
        self.worker.spawn(move |git| {
            let left = git.file_at(&root, "HEAD", &path.to_string_lossy()).map(Option::unwrap_or_default);
            let right = std::fs::read(&path).map(|bytes| decode(&bytes).content).unwrap_or_default();
            Event::Compare(Loaded::Pair(generation, left.map(|left| (left, right))))
        });
    }

    /// File cambiati da un commit, con il diff di quello scelto nella lista.
    pub fn show_commit(&self, root: &Path, hash: &str, subject: &str, font: HFONT) {
        let short = hash.get(..7).unwrap_or(hash);
        self.open(Mode::Commit, root, format!("{short} · {subject}"), ["", "", ""], font);
        self.state.borrow_mut().hash = hash.to_owned();
        let (root, hash) = (root.to_string_lossy().into_owned(), hash.to_owned());
        self.worker.spawn(move |git| {
            let files = git.commit_changes(&root, &hash);
            Event::Compare(Loaded::Files(hash, files))
        });
    }

    /// Commit che hanno toccato il file (o, con `lines`, quelle righe), con il diff di quello scelto.
    pub fn show_history(&self, root: &Path, path: &Path, lines: Option<(u32, u32)>, font: HFONT) {
        let title = format!("History di {}{}", file_name(path), if lines.is_some() { " (selezione)" } else { "" });
        self.open(Mode::History, root, title, ["Prima", "Dopo", ""], font);
        {
            let mut state = self.state.borrow_mut();
            state.path = path.to_path_buf();
            state.lines = lines;
            state.only_changes = lines.is_some();
            state.subtitle = path.strip_prefix(root).map_or_else(|_| path.display().to_string(), |relative| relative.display().to_string());
        }
        self.files.set_row_height(self.scale(ROW_HEIGHT) * 2);
        let (root, path) = (root.to_string_lossy().into_owned(), path.to_path_buf());
        self.worker.spawn(move |git| {
            let history = git.file_history(&root, &path.to_string_lossy(), lines);
            Event::Compare(Loaded::History(path, history))
        });
    }

    fn select_revision(&self, index: usize) {
        let (root, revision) = {
            let state = self.state.borrow();
            let Some(revision) = state.revisions.get(index) else { return };
            (state.root.to_string_lossy().into_owned(), (revision.commit.hash.clone(), revision.commit.parents.is_empty(), revision.path.clone(), revision.old_path.clone()))
        };
        let (hash, first, path, old_path) = revision;
        let short = hash.get(..7).unwrap_or(&hash).to_owned();
        {
            let mut state = self.state.borrow_mut();
            state.labels = [if first { "(file nuovo)".into() } else { "Prima".into() }, format!("Dopo ({short})"), String::new()];
            state.path = PathBuf::from(&path);
        }
        self.invalidate();
        let generation = self.next_generation();
        self.worker.spawn(move |git| {
            let before = if first { Ok(None) } else { git.file_at(&root, &format!("{hash}^"), old_path.as_deref().unwrap_or(&path)) };
            let after = git.file_at(&root, &hash, &path);
            let pair = before.and_then(|before| after.map(|after| (before.unwrap_or_default(), after.unwrap_or_default())));
            Event::Compare(Loaded::Pair(generation, pair))
        });
    }

    /// Il file che il dialogo ha chiesto di aprire nell'editor.
    pub fn take_open_request(&self) -> Option<PathBuf> {
        self.open_request.borrow_mut().take()
    }

    /// Merge a 3 vie di un file in conflitto.
    pub fn show_merge(&self, root: &Path, path: &Path, font: HFONT) {
        self.open(Mode::Merge, root, format!("Conflitti in {}", file_name(path)), ["Nostra (HEAD)", "Result", "Loro"], font);
        self.state.borrow_mut().path = path.to_path_buf();
        let (root, path) = (root.to_string_lossy().into_owned(), path.to_path_buf());
        self.worker.spawn(move |git| {
            let versions = git.conflict_versions(&root, &path.to_string_lossy());
            let current = std::fs::read(&path).map(|bytes| decode(&bytes)).map_err(|error| error.to_string());
            Event::Compare(Loaded::Conflict(path, versions.and_then(|versions| current.map(|current| (versions, current)))))
        });
    }

    fn next_generation(&self) -> u64 {
        self.generation.set(self.generation.get() + 1);
        self.generation.get()
    }

    pub fn on_loaded(&self, loaded: Loaded) {
        // SAFETY: finestra valida.
        if unsafe { IsWindowVisible(self.hwnd) } == 0 {
            return;
        }
        match loaded {
            Loaded::Files(hash, Ok(files)) if hash == self.state.borrow().hash => {
                let count = files.len();
                self.state.borrow_mut().files = files;
                self.files.set_rows(count);
                if count > 0 {
                    self.files.select(0, ListEvent::Select);
                }
            }
            Loaded::Pair(generation, Ok((left, right))) if generation == self.generation.get() => self.show_pair(&left, &right),
            Loaded::Conflict(path, Ok((versions, current))) if path == self.state.borrow().path => self.show_conflict(versions, current),
            Loaded::History(path, Ok(revisions)) if self.state.borrow().mode == Mode::History && path == self.state.borrow().path => {
                let count = revisions.len();
                self.state.borrow_mut().revisions = revisions;
                self.files.set_rows(count);
                if count > 0 {
                    self.files.select(0, ListEvent::Select);
                }
                self.invalidate();
            }
            Loaded::Files(_, Err(error)) | Loaded::Pair(_, Err(error)) | Loaded::Conflict(_, Err(error)) | Loaded::History(_, Err(error)) => {
                message_box(self.hwnd, &error, MB_ICONERROR);
            }
            _ => {}
        }
    }

    fn select_file(&self, index: usize) {
        let (root, hash, file) = {
            let state = self.state.borrow();
            let Some(file) = state.files.get(index) else { return };
            (state.root.to_string_lossy().into_owned(), state.hash.clone(), (file.path.clone(), file.old_path.clone(), file.status.clone()))
        };
        let short = hash.get(..7).unwrap_or(&hash).to_owned();
        {
            let mut state = self.state.borrow_mut();
            state.labels = [format!("{short}^ (prima)"), format!("{short} (dopo)"), String::new()];
            state.path = PathBuf::from(&file.0);
        }
        self.invalidate();
        let generation = self.next_generation();
        self.worker.spawn(move |git| {
            let (path, old_path, status) = file;
            let before = if status == "A" { Ok(None) } else { git.file_at(&root, &format!("{hash}^1"), old_path.as_deref().unwrap_or(&path)) };
            let after = if status == "D" { Ok(None) } else { git.file_at(&root, &hash, &path) };
            let pair = before.and_then(|before| after.map(|after| (before.unwrap_or_default(), after.unwrap_or_default())));
            Event::Compare(Loaded::Pair(generation, pair))
        });
    }

    fn set_text(&self, sci: &Sci, text: &str, language: &'static lang::Language) {
        side_by_side::set_text(sci, text, language, &self.palette, self.font_fraction.get());
    }

    fn show_pair(&self, left: &str, right: &str) {
        let path = self.state.borrow().path.clone();
        let (stops, kinds) = side_by_side::show([&self.panes[0], &self.panes[1]], left, right, &path, &self.palette, self.font_fraction.get());
        let only = self.state.borrow().only_changes;
        side_by_side::only_changes([&self.panes[0], &self.panes[1]], &kinds, only);
        {
            let mut state = self.state.borrow_mut();
            state.stops = stops;
            state.kinds = kinds;
        }
        self.jump(1);
    }

    fn show_conflict(&self, versions: ConflictVersions, current: TextFile) {
        let path = self.state.borrow().path.clone();
        let language = lang::detect(&path);
        let base = versions.base.unwrap_or_default();
        for (sci, text) in [(&self.panes[0], versions.ours.unwrap_or_default()), (&self.panes[2], versions.theirs.unwrap_or_default())] {
            self.set_text(sci, &text, language);
            sci.call(SCI_SETMARGINTYPEN, 0, SC_MARGIN_NUMBER);
            for (line, kind) in changed_lines(&base, &text).iter().enumerate() {
                if let Some(&(_, marker)) = MARKERS.iter().find(|(marker_kind, _)| marker_kind == kind) {
                    sci.call(SCI_MARKERADD, line, marker as isize);
                }
            }
            sci.call(SCI_SETREADONLY, 1, 0);
        }
        let result = &self.panes[1];
        self.set_text(result, &current.content, language);
        result.call(SCI_SETMARGINTYPEN, 0, SC_MARGIN_NUMBER);
        let eol = if current.content.contains("\r\n") { SC_EOL_CRLF } else { SC_EOL_LF };
        result.call(SCI_SETEOLMODE, eol, 0);
        self.state.borrow_mut().merge = Some(MergeFile { path, encoding: current.encoding, bom: current.bom });
        self.mark_conflicts();
        self.jump(1);
        // SAFETY: controllo valido.
        unsafe { SetFocus(result.hwnd) };
    }

    /// Ricalcola i blocchi di conflitto del risultato dopo ogni modifica.
    fn mark_conflicts(&self) {
        let result = &self.panes[1];
        let text = result.text();
        result.call(SCI_MARKERDELETEALL, MARKER_CONFLICT, 0);
        let mut stops = Vec::new();
        for conflict in parse_conflicts(&text) {
            let first = result.call(SCI_LINEFROMPOSITION, conflict.from, 0);
            let last = result.call(SCI_LINEFROMPOSITION, conflict.to.saturating_sub(1), 0);
            for line in first..=last {
                result.call(SCI_MARKERADD, line as usize, MARKER_CONFLICT as isize);
            }
            stops.push(first as usize);
        }
        let changed = self.state.borrow().stops.len() != stops.len();
        self.state.borrow_mut().stops = stops;
        if changed {
            self.invalidate();
        }
    }

    /// F7 / Shift+F7: modifica (o conflitto) successiva o precedente rispetto al cursore.
    fn jump(&self, direction: i32) {
        let state = self.state.borrow();
        let sci = if state.mode == Mode::Merge { &self.panes[1] } else { &self.panes[0] };
        let caret = sci.call(SCI_LINEFROMPOSITION, sci.call(SCI_GETCURRENTPOS, 0, 0) as usize, 0) as usize;
        let target = if direction > 0 {
            state.stops.iter().find(|&&line| line > caret || (caret == 0 && line == 0 && sci.call(SCI_GETCURRENTPOS, 0, 0) == 0))
        } else {
            state.stops.iter().rev().find(|&&line| line < caret)
        };
        let Some(&line) = target else { return };
        let mode = state.mode;
        drop(state);
        let panes = if mode == Mode::Merge { &self.panes[1..2] } else { &self.panes[..2] };
        for sci in panes {
            sci.call(SCI_GOTOLINE, line, 0);
            sci.call(SCI_SETFIRSTVISIBLELINE, line.saturating_sub(5), 0);
        }
    }

    fn resolve_all(&self, ours: bool) {
        let result = &self.panes[1];
        let resolved = resolve_all(&result.text(), ours);
        result.call(SCI_SETTARGETRANGE, 0, result.call(SCI_GETLENGTH, 0, 0));
        result.call(SCI_REPLACETARGET, resolved.len(), resolved.as_ptr() as isize);
        self.mark_conflicts();
    }

    fn apply(&self) {
        let remaining = self.state.borrow().stops.len();
        if remaining > 0 {
            let question = format!("Restano {remaining} conflitti non risolti. Salvare comunque e segnare il file come risolto?");
            if message_box(self.hwnd, &question, MB_YESNO | MB_ICONWARNING) != IDYES {
                return;
            }
        }
        let state = self.state.borrow();
        let Some(merge) = state.merge.as_ref() else { return };
        let written = encode(&self.panes[1].text(), &merge.encoding, merge.bom).and_then(|bytes| std::fs::write(&merge.path, bytes).map_err(|error| error.to_string()));
        if let Err(error) = written {
            message_box(self.hwnd, &format!("Salvataggio non riuscito:\n{error}"), MB_ICONERROR);
            return;
        }
        let path = merge.path.to_string_lossy().into_owned();
        self.worker.run(&state.root, move |git, root| git.stage(root, &[path]));
        drop(state);
        self.hide();
    }

    /// Esc, Annulla, chiusura: nel merge chiede conferma se il risultato è stato modificato.
    fn cancel(&self) {
        let merging = self.state.borrow().mode == Mode::Merge;
        if merging && self.panes[1].call(SCI_GETMODIFY, 0, 0) != 0 && message_box(self.hwnd, "Chiudere senza salvare le risoluzioni fatte?", MB_YESNO | MB_ICONWARNING) != IDYES {
            return;
        }
        self.hide();
    }

    fn hide(&self) {
        self.next_generation();
        self.state.borrow_mut().hash.clear();
        for pane in &self.panes {
            self.set_text(pane, "", &lang::PLAIN);
        }
        // SAFETY: finestre valide; il focus torna alla finestra principale.
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            SetForegroundWindow(self.owner);
        }
    }

    /// Tasti della finestra: Esc chiude, F7 e Shift+F7 scorrono le modifiche. Gli altri vanno ai controlli.
    pub fn key(&self, msg: &MSG) -> bool {
        if msg.message != WM_KEYDOWN {
            return false;
        }
        match msg.wParam as u16 {
            VK_ESCAPE => self.cancel(),
            VK_F7 => {
                // SAFETY: lettura dello stato della tastiera del thread.
                let shift = unsafe { GetKeyState(VK_SHIFT as i32) } < 0;
                self.jump(if shift { -1 } else { 1 });
            }
            _ => return false,
        }
        true
    }

    fn layout(&self) {
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: finestra valida.
        unsafe { GetClientRect(self.hwnd, &mut client) };
        let mode = self.state.borrow().mode;
        let top = self.scale(HEADER + LABELS);
        let mut left = 0;
        let place = |hwnd: HWND, shown: bool, rect: RECT| {
            // SAFETY: controlli figli validi.
            unsafe {
                ShowWindow(hwnd, if shown { SW_SHOW } else { SW_HIDE });
                MoveWindow(hwnd, rect.left, rect.top, (rect.right - rect.left).max(0), (rect.bottom - rect.top).max(0), 1);
            }
        };
        let with_files = matches!(mode, Mode::Commit | Mode::History);
        if with_files {
            left = self.scale(if mode == Mode::History { HISTORY_WIDTH } else { FILES_WIDTH });
        }
        place(self.files.hwnd, with_files, RECT { left: 0, top, right: left - 1, bottom: client.bottom });
        let count = if mode == Mode::Merge { 3 } else { 2 };
        let width = (client.right - left) / count;
        for (index, pane) in self.panes.iter().enumerate() {
            let x = left + index as i32 * width;
            let right = if index as i32 == count - 1 { client.right } else { x + width - 1 };
            place(pane.hwnd, (index as i32) < count, RECT { left: x, top, right, bottom: client.bottom });
        }
    }

    fn pane_left(&self, index: usize) -> i32 {
        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: controlli figli validi; coordinate riportate nella finestra.
        unsafe {
            GetWindowRect(self.panes[index].hwnd, &mut rect);
            let mut point = windows_sys::Win32::Foundation::POINT { x: rect.left, y: rect.top };
            ScreenToClient(self.hwnd, &mut point);
            point.x
        }
    }

    fn invalidate(&self) {
        // SAFETY: finestra valida.
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    unsafe fn paint(&self) {
        let mut paint: PAINTSTRUCT = std::mem::zeroed();
        let target = BeginPaint(self.hwnd, &mut paint);
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(self.hwnd, &mut client);
        let dc = CreateCompatibleDC(target);
        let bitmap = CreateCompatibleBitmap(target, client.right.max(1), client.bottom.max(1));
        let previous_bitmap = SelectObject(dc, bitmap);
        let previous_font = SelectObject(dc, self.font.get());
        SetBkMode(dc, TRANSPARENT as i32);
        let palette = &self.palette;
        fill(dc, &client, palette.panel);
        let header = RECT { bottom: self.scale(HEADER), ..client };
        let labels = RECT { top: header.bottom, bottom: header.bottom + self.scale(LABELS), ..client };
        fill(dc, &RECT { top: labels.bottom - 1, bottom: labels.bottom, ..client }, palette.border);
        fill(dc, &RECT { top: header.bottom - 1, bottom: header.bottom, ..client }, palette.border);

        let state = self.state.borrow();
        let entries: &[(&str, Button)] = match state.mode {
            Mode::Merge => &[
                ("◀ Precedente", Button::Previous),
                ("Successivo ▶", Button::Next),
                ("Tutte nostre", Button::AllOurs),
                ("Tutte loro", Button::AllTheirs),
                ("Applica e segna risolto", Button::Apply),
                ("Annulla", Button::Cancel),
            ],
            Mode::Diff => &[("Apri nell'editor", Button::OpenInEditor), ("Chiudi", Button::Cancel)],
            Mode::History => &[("Solo le zone cambiate", Button::OnlyChanges), ("Chiudi", Button::Cancel)],
            Mode::Commit => &[("Chiudi", Button::Cancel)],
        };
        let mut right = header.right - self.scale(10);
        let mut buttons = Vec::new();
        for &(label, button) in entries.iter().rev() {
            let check = if button == Button::OnlyChanges { self.scale(20) } else { 0 };
            let width = text_width(dc, label) + self.scale(20) + check;
            let area = RECT { left: right - width, top: header.top + self.scale(7), right, bottom: header.bottom - self.scale(7) };
            if button == Button::OnlyChanges {
                let size = self.scale(14);
                let middle = (area.top + area.bottom) / 2;
                let box_rect = RECT { left: area.left + self.scale(6), top: middle - size / 2, right: area.left + self.scale(6) + size, bottom: middle + size / 2 };
                frame(dc, &box_rect, palette.muted);
                if state.only_changes {
                    fill(dc, &RECT { left: box_rect.left + 3, top: box_rect.top + 3, right: box_rect.right - 3, bottom: box_rect.bottom - 3 }, palette.accent);
                }
                text(dc, label, &RECT { left: box_rect.right + self.scale(6), ..area }, palette.fg, DT_LEFT);
            } else if button == Button::Apply {
                fill(dc, &area, palette.accent);
                text(dc, label, &area, palette.editor, DT_CENTER);
            } else {
                frame(dc, &area, palette.border);
                text(dc, label, &area, palette.fg, DT_CENTER);
            }
            buttons.push((area, button));
            right = area.left - self.scale(6);
        }
        let title = state.title.clone();
        let title_width = text_width(dc, &title).min(right - self.scale(24));
        text(dc, &title, &RECT { left: header.left + self.scale(12), right: header.left + self.scale(12) + title_width, ..header }, palette.fg, DT_LEFT | DT_END_ELLIPSIS);
        if state.mode == Mode::Merge && state.merge.is_some() {
            let remaining = state.stops.len();
            let (label, color) = if remaining == 0 { ("tutti risolti".to_string(), palette.git_added) } else { (format!("{remaining} da risolvere"), palette.danger) };
            text(dc, &label, &RECT { left: header.left + self.scale(20) + title_width, right: right - self.scale(12), ..header }, color, DT_LEFT | DT_END_ELLIPSIS);
        }
        text(dc, &state.subtitle, &RECT { left: header.left + self.scale(22) + title_width, right: right - self.scale(12), ..header }, palette.muted, DT_LEFT | DT_PATH_ELLIPSIS);
        *self.buttons.borrow_mut() = buttons;

        let count = if state.mode == Mode::Merge { 3 } else { 2 };
        for index in 0..count {
            let left = self.pane_left(index) + self.scale(8);
            let right = if index + 1 < count { self.pane_left(index + 1) } else { client.right };
            text(dc, &state.labels[index], &RECT { left, right, ..labels }, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
        }
        if state.mode == Mode::Commit {
            let area = RECT { left: self.scale(8), right: self.scale(FILES_WIDTH), ..labels };
            text(dc, &format!("{} file cambiati", state.files.len()), &area, palette.muted, DT_LEFT);
        }
        if state.mode == Mode::History {
            let area = RECT { left: self.scale(10), right: self.scale(HISTORY_WIDTH), ..labels };
            let count = if state.revisions.len() == 1 { "1 commit".to_string() } else { format!("{} commit", state.revisions.len()) };
            let range = state.lines.map_or_else(String::new, |(from, to)| format!(" · righe {from}–{to}"));
            text(dc, &format!("{count}{range}"), &area, palette.muted, DT_LEFT);
        }

        BitBlt(target, 0, 0, client.right, client.bottom, dc, 0, 0, SRCCOPY);
        SelectObject(dc, previous_font);
        SelectObject(dc, previous_bitmap);
        DeleteObject(bitmap);
        DeleteDC(dc);
        EndPaint(self.hwnd, &paint);
    }

    unsafe fn paint_file_row(&self, row: &RowPaint) {
        let state = self.state.borrow();
        let palette = &self.palette;
        let dc = row.dc;
        SelectObject(dc, self.font.get());
        SetBkMode(dc, TRANSPARENT as i32);
        if row.selected {
            fill(dc, &row.rect, if row.focused { palette.selection } else { palette.active_line });
        }
        // History: oggetto, poi hash · autore · data relativa, su due righe come nella versione Tauri.
        if state.mode == Mode::History {
            let Some(revision) = state.revisions.get(row.index) else { return };
            let commit = &revision.commit;
            let middle = (row.rect.top + row.rect.bottom) / 2;
            let left = row.rect.left + self.scale(10);
            text(dc, &commit.subject, &RECT { left, top: row.rect.top + self.scale(2), right: row.rect.right - self.scale(8), bottom: middle }, palette.fg, DT_LEFT | DT_END_ELLIPSIS);
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs() as i64);
            let meta = format!("{} · {} · {}", commit.short_hash, commit.author, crate::panel::relative_time(commit.timestamp, now));
            text(dc, &meta, &RECT { left, top: middle, right: row.rect.right - self.scale(8), bottom: row.rect.bottom - self.scale(2) }, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
            return;
        }
        let Some(file) = state.files.get(row.index) else { return };
        let color = match file.status.as_str() {
            "A" => palette.git_added,
            "D" => palette.git_ignored,
            _ => palette.git_modified,
        };
        let relative = Path::new(&file.path).strip_prefix(&state.root).map_or_else(|_| file.path.clone(), |path| path.to_string_lossy().into_owned());
        let left = row.rect.left + self.scale(8);
        text(dc, &file.status, &RECT { left, right: left + self.scale(16), ..row.rect }, color, DT_LEFT);
        text(dc, &relative, &RECT { left: left + self.scale(20), right: row.rect.right - self.scale(6), ..row.rect }, color, DT_LEFT | DT_PATH_ELLIPSIS);
    }

    fn on_click(&self, x: i32, y: i32) {
        let button = self.buttons.borrow().iter().find(|(rect, _)| contains(rect, x, y)).map(|(_, button)| *button);
        match button {
            Some(Button::Previous) => self.jump(-1),
            Some(Button::Next) => self.jump(1),
            Some(Button::AllOurs) => self.resolve_all(true),
            Some(Button::AllTheirs) => self.resolve_all(false),
            Some(Button::Apply) => self.apply(),
            Some(Button::Cancel) => self.cancel(),
            Some(Button::OpenInEditor) => {
                let path = self.state.borrow().path.clone();
                *self.open_request.borrow_mut() = Some(path);
                self.hide();
                // SAFETY: messaggio alla finestra principale, che apre il file.
                unsafe { PostMessageW(self.owner, WM_COMPARE_OPEN, 0, 0) };
            }
            Some(Button::OnlyChanges) => {
                let (on, kinds) = {
                    let mut state = self.state.borrow_mut();
                    state.only_changes = !state.only_changes;
                    (state.only_changes, state.kinds.clone())
                };
                side_by_side::only_changes([&self.panes[0], &self.panes[1]], &kinds, on);
                self.invalidate();
            }
            None => {}
        }
    }

    /// Diff: i due lati hanno le stesse righe, si scorrono insieme.
    fn on_scroll(&self, source: HWND) {
        if self.state.borrow().mode == Mode::Merge {
            return;
        }
        side_by_side::sync_scroll([&self.panes[0], &self.panes[1]], source);
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(compare) = compare() else { return DefWindowProcW(hwnd, message, wparam, lparam) };
    match message {
        WM_SIZE => {
            compare.layout();
            compare.invalidate();
            0
        }
        WM_PAINT => {
            compare.paint();
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            compare.on_click((lparam & 0xFFFF) as i16 as i32, ((lparam >> 16) & 0xFFFF) as i16 as i32);
            0
        }
        WM_NOTIFY => {
            let header = &*(lparam as *const NMHDR);
            match header.code {
                SCN_UPDATEUI => compare.on_scroll(header.hwndFrom),
                SCN_MODIFIED if header.hwndFrom == compare.panes[1].hwnd && compare.state.borrow().mode == Mode::Merge => compare.mark_conflicts(),
                _ => {}
            }
            0
        }
        WM_LIST_PAINT => {
            compare.paint_file_row(&*(lparam as *const RowPaint));
            0
        }
        WM_LIST_EVENT => {
            let (event, index) = decode_event(lparam);
            if event == ListEvent::Select {
                if compare.state.borrow().mode == Mode::History {
                    compare.select_revision(index);
                } else {
                    compare.select_file(index);
                }
            }
            0
        }
        WM_CLOSE => {
            compare.cancel();
            0
        }
        WM_ACTIVATE if (wparam & 0xFFFF) as u32 == WA_INACTIVE && lparam as HWND == compare.owner => {
            if compare.state.borrow().mode != Mode::Merge {
                compare.hide();
            }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}


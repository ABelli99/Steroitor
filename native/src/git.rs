//! Git in background: ogni operazione gira su un thread, il risultato torna alla finestra con un messaggio.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use steroitor_core::git::parse::{Branch, FileStatus, RepoStatus};
use steroitor_core::git::runner::{CommandLog, Git};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_GIT_EVENT: u32 = WM_APP + 2;
pub const WM_COMMAND_LOGGED: u32 = WM_APP + 3;
const LOG_LIMIT: usize = 500;

pub enum Event {
    Status(PathBuf, Result<RepoStatus, String>),
    Branches(PathBuf, Vec<Branch>),
    Head(PathBuf, Option<String>),
    /// Un'operazione voluta dall'utente (pull, checkout, …): l'errore va mostrato.
    Finished(PathBuf, Result<(), String>),
}

#[derive(Clone)]
pub struct Worker {
    hwnd: usize,
    events: Arc<Mutex<Vec<Event>>>,
    /// I comandi git eseguiti, per la Console.
    pub log: Arc<Mutex<Vec<CommandLog>>>,
}

impl Worker {
    pub fn new(hwnd: HWND) -> Worker {
        Worker { hwnd: hwnd as usize, events: Arc::default(), log: Arc::default() }
    }

    fn post(hwnd: usize, message: u32) {
        // SAFETY: PostMessageW è sicura da qualsiasi thread; se la finestra non esiste più non fa nulla.
        unsafe { PostMessageW(hwnd as HWND, message, 0, 0) };
    }

    pub fn spawn(&self, job: impl FnOnce(&Git) -> Event + Send + 'static) {
        let worker = self.clone();
        std::thread::spawn(move || {
            let log = Arc::clone(&worker.log);
            let hwnd = worker.hwnd;
            let git = Git::new(
                "git",
                Box::new(move |entry| {
                    if let Ok(mut log) = log.lock() {
                        if log.len() == LOG_LIMIT {
                            log.remove(0);
                        }
                        log.push(entry);
                    }
                    Self::post(hwnd, WM_COMMAND_LOGGED);
                }),
            );
            let event = job(&git);
            if let Ok(mut events) = worker.events.lock() {
                events.push(event);
            }
            Self::post(worker.hwnd, WM_GIT_EVENT);
        });
    }

    pub fn take(&self) -> Vec<Event> {
        self.events.lock().map(|mut events| std::mem::take(&mut *events)).unwrap_or_default()
    }

    pub fn refresh(&self, root: &Path) {
        let path = root.to_path_buf();
        self.spawn(move |git| Event::Status(path.clone(), git.status(&path.to_string_lossy())));
        let path = root.to_path_buf();
        self.spawn(move |git| Event::Branches(path.clone(), git.branches(&path.to_string_lossy()).unwrap_or_default()));
    }

    pub fn head(&self, root: &Path, file: &Path) {
        let (root, file) = (root.to_path_buf(), file.to_path_buf());
        self.spawn(move |git| {
            let content = git.head_content(&root.to_string_lossy(), &file.to_string_lossy()).ok().flatten().map(|file| file.content);
            Event::Head(file, content)
        });
    }

    pub fn run(&self, root: &Path, task: impl FnOnce(&Git, &str) -> Result<(), String> + Send + 'static) {
        let root = root.to_path_buf();
        self.spawn(move |git| {
            let result = task(git, &root.to_string_lossy());
            Event::Finished(root, result)
        });
    }
}

pub struct Repo {
    pub root: PathBuf,
    pub status: Option<RepoStatus>,
    pub branches: Vec<Branch>,
}

/// Stato Git dei file, per colorarli: percorso (minuscolo) → stato; le cartelle con modifiche dentro risultano modificate.
#[derive(Default)]
pub struct FileColors {
    files: HashMap<String, FileStatus>,
    dirs: HashSet<String>,
}

pub fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

impl FileColors {
    pub fn build(repos: &[Repo]) -> FileColors {
        let mut colors = FileColors::default();
        for status in repos.iter().filter_map(|repo| repo.status.as_ref()) {
            for entry in &status.files {
                let path = PathBuf::from(&entry.path);
                colors.files.insert(key(&path), entry.status);
                if entry.status == FileStatus::Ignored {
                    continue;
                }
                for dir in path.ancestors().skip(1) {
                    if !colors.dirs.insert(key(dir)) {
                        break;
                    }
                }
            }
        }
        colors
    }

    pub fn of(&self, path: &Path) -> Option<FileStatus> {
        let key = key(path);
        if let Some(status) = self.files.get(&key) {
            return Some(*status);
        }
        // Una cartella ignorata o non tracciata compare in `git status` come `cartella/`.
        if let Some(status) = self.files.get(&format!("{key}\\")) {
            return Some(*status);
        }
        self.dirs.contains(&key).then_some(FileStatus::Modified)
    }
}

/// Repository che contiene `path`: il più interno, per le cartelle con più repository annidati.
pub fn repo_of<'a>(repos: &'a [Repo], path: &Path) -> Option<&'a Repo> {
    let key = key(path);
    repos.iter().filter(|repo| key.starts_with(&self::key(&repo.root))).max_by_key(|repo| repo.root.as_os_str().len())
}

/// Righe cambiate rispetto a HEAD, per i marker nel gutter (numeri di riga 0-based del testo attuale).
#[derive(Debug, PartialEq, Default)]
pub struct LineChanges {
    pub added: Vec<usize>,
    pub modified: Vec<usize>,
    /// Righe sotto le quali sono state tolte righe di HEAD.
    pub deleted: Vec<usize>,
}

pub fn line_changes(head: &str, current: &str) -> LineChanges {
    use similar::{DiffOp, TextDiff};
    let (head, current) = (head.replace("\r\n", "\n"), current.replace("\r\n", "\n"));
    let diff = TextDiff::configure().timeout(std::time::Duration::from_millis(200)).diff_lines(&head, &current);
    let mut changes = LineChanges::default();
    for op in diff.ops() {
        match *op {
            DiffOp::Insert { new_index, new_len, .. } => changes.added.extend(new_index..new_index + new_len),
            DiffOp::Replace { new_index, new_len, .. } => changes.modified.extend(new_index..new_index + new_len),
            DiffOp::Delete { new_index, .. } => changes.deleted.push(new_index),
            DiffOp::Equal { .. } => {}
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_changed_lines_ignoring_line_endings() {
        let head = "a\nb\nc\nd\n";
        let current = "a\r\nB\r\nc\r\nnew\r\n";
        let changes = line_changes(head, current);
        assert_eq!(changes.modified, vec![1, 3]);
        assert!(changes.added.is_empty());
        let changes = line_changes("a\nb\n", "a\nx\nb\n");
        assert_eq!(changes.added, vec![1]);
        let changes = line_changes("a\nb\nc\n", "a\nc\n");
        assert_eq!(changes.deleted, vec![1]);
    }

    #[test]
    fn colors_folders_that_contain_changes() {
        let status = steroitor_core::git::parse::parse_status(Path::new("C:\\repo"), "# branch.head main\x001 .M N... 100644 100644 100644 a b src/app.rs\x00? notes.txt\x00");
        let colors = FileColors::build(&[Repo { root: PathBuf::from("C:\\repo"), status: Some(status), branches: Vec::new() }]);
        assert_eq!(colors.of(Path::new("C:\\Repo\\src\\app.rs")), Some(FileStatus::Modified));
        assert_eq!(colors.of(Path::new("C:\\repo\\src")), Some(FileStatus::Modified));
        assert_eq!(colors.of(Path::new("C:\\repo\\notes.txt")), Some(FileStatus::Untracked));
        assert_eq!(colors.of(Path::new("C:\\repo\\other.txt")), None);
    }
}

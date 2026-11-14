//! Git in background: ogni operazione gira su un thread, il risultato torna alla finestra con un messaggio.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use steroitor_core::git::history::LogFilter;
use steroitor_core::git::parse::{BlameLine, Branch, Commit, FileStatus, RepoStatus};
use steroitor_core::git::runner::{CommandLog, Git};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_GIT_EVENT: u32 = WM_APP + 2;
pub const WM_COMMAND_LOGGED: u32 = WM_APP + 3;
/// Commit caricati nel log: abbastanza per la storia recente senza pesare in memoria.
const LOG_COMMITS: u32 = 1000;

pub enum Event {
    Status(PathBuf, Result<RepoStatus, String>),
    Branches(PathBuf, Vec<Branch>),
    Head(PathBuf, Option<String>),
    /// Un'operazione voluta dall'utente (pull, checkout, …): l'errore va mostrato.
    Finished(PathBuf, Result<(), String>),
    Log(PathBuf, Result<Vec<Commit>, String>),
    Committed(PathBuf, Result<(), String>),
    /// Il commit si è fermato prima di committare: questi file in stage hanno fine riga CRLF.
    Crlf(PathBuf, Vec<String>, CommitRequest),
    Cloned(PathBuf, Result<(), String>),
    Blame(PathBuf, Result<Vec<BlameLine>, String>),
    /// File cambiati da un commit, per i dettagli nella tab Git: repository, hash, file.
    CommitFiles(PathBuf, String, Result<Vec<steroitor_core::git::changes::ChangedFile>, String>),
    Compare(crate::compare::Loaded),
}

/// Cosa committare: file da aggiungere e da togliere dall'index, messaggio, push, amend dell'ultimo commit.
#[derive(Clone)]
pub struct CommitRequest {
    pub files: Vec<String>,
    pub unstage: Vec<String>,
    pub message: String,
    pub push: bool,
    pub amend: bool,
}

#[derive(Clone)]
pub struct Worker {
    hwnd: usize,
    events: Arc<Mutex<Vec<Event>>>,
    /// I comandi git eseguiti, in attesa che la Console li mostri.
    log: Arc<Mutex<Vec<CommandLog>>>,
}

impl Worker {
    pub fn new(hwnd: HWND) -> Worker {
        Worker { hwnd: hwnd as usize, events: Arc::default(), log: Arc::default() }
    }

    fn post(hwnd: usize, message: u32) {
        // SAFETY: PostMessageW è sicura da qualsiasi thread; se la finestra non esiste più non fa nulla.
        unsafe { PostMessageW(hwnd as HWND, message, 0, 0) };
    }

    /// Git che registra i comandi per la Console.
    fn git(&self) -> Git {
        let log = Arc::clone(&self.log);
        let hwnd = self.hwnd;
        Git::new(
            "git",
            Box::new(move |entry| {
                if let Ok(mut log) = log.lock() {
                    log.push(entry);
                }
                Self::post(hwnd, WM_COMMAND_LOGGED);
            }),
        )
    }

    /// Comando rapido sul thread della UI, quando serve la risposta prima di continuare (es. il testo di un messaggio).
    pub fn now<T>(&self, task: impl FnOnce(&Git) -> T) -> T {
        task(&self.git())
    }

    pub fn spawn(&self, job: impl FnOnce(&Git) -> Event + Send + 'static) {
        let worker = self.clone();
        std::thread::spawn(move || {
            let git = worker.git();
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

    pub fn take_log(&self) -> Vec<CommandLog> {
        self.log.lock().map(|mut log| std::mem::take(&mut *log)).unwrap_or_default()
    }

    pub fn log(&self, root: &Path, filter: LogFilter) {
        let root = root.to_path_buf();
        self.spawn(move |git| Event::Log(root.clone(), git.log_filtered(&root.to_string_lossy(), &filter, 0, LOG_COMMITS)));
    }

    pub fn commit_files(&self, root: &Path, hash: String) {
        let root = root.to_path_buf();
        self.spawn(move |git| {
            let files = git.commit_changes(&root.to_string_lossy(), &hash);
            Event::CommitFiles(root, hash, files)
        });
    }

    pub fn blame(&self, root: &Path, file: &Path) {
        let (root, file) = (root.to_path_buf(), file.to_path_buf());
        self.spawn(move |git| {
            let lines = git.blame(&root.to_string_lossy(), &file.to_string_lossy());
            Event::Blame(file, lines)
        });
    }

    /// Commit dei file indicati: toglie dall'index quelli non scelti, aggiunge gli altri, committa (o fa amend) ed eventualmente push.
    /// Con `check_crlf` si ferma dopo lo stage se ci sono file con fine riga CRLF, per chiedere all'utente.
    pub fn commit(&self, root: &Path, request: CommitRequest, check_crlf: bool) {
        let root = root.to_path_buf();
        self.spawn(move |git| {
            let path = root.to_string_lossy().into_owned();
            let staged = (|| {
                if !request.unstage.is_empty() {
                    git.unstage(&path, &request.unstage)?;
                }
                if !request.files.is_empty() {
                    git.stage(&path, &request.files)?;
                }
                if check_crlf {
                    return git.staged_crlf_files(&path);
                }
                Ok(Vec::new())
            })();
            match staged {
                Ok(crlf) if !crlf.is_empty() => return Event::Crlf(root, crlf, request),
                Err(error) => return Event::Committed(root, Err(error)),
                Ok(_) => {}
            }
            let result = (|| {
                if request.amend {
                    git.amend(&path, &request.message)?;
                } else {
                    git.commit(&path, &request.message)?;
                }
                if request.push {
                    git.push(&path)?;
                }
                Ok(())
            })();
            Event::Committed(root, result)
        });
    }

    pub fn clone_repo(&self, url: String, target: PathBuf) {
        self.spawn(move |git| {
            let result = git.clone_repo(&url, &target.to_string_lossy());
            Event::Cloned(target, result)
        });
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

/// Nome della cartella di un clone: l'ultimo pezzo dell'URL senza `.git`.
pub fn repo_name(url: &str) -> Option<String> {
    let name = url.trim().trim_end_matches(['/', '\\']).rsplit(['/', '\\', ':']).next()?.trim_end_matches(".git");
    (!name.is_empty()).then(|| name.to_string())
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
    fn names_the_clone_after_the_url() {
        assert_eq!(repo_name("https://github.com/org/steroitor.git").as_deref(), Some("steroitor"));
        assert_eq!(repo_name("git@github.com:org/tool.git/").as_deref(), Some("tool"));
        assert_eq!(repo_name("C:\\repos\\local").as_deref(), Some("local"));
        assert_eq!(repo_name("   "), None);
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

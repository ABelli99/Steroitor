use crate::git::discover::find_repos;
use serde::Serialize;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, State, WebviewWindow};

const DEBOUNCE: Duration = Duration::from_millis(200);
const ALWAYS_SKIPPED: &[&str] = &[".git", "node_modules"];

struct Watch {
    window: String,
    _debouncer: Debouncer<RecommendedWatcher>,
}

/// Un watcher per progetto; gli eventi vanno alla finestra che lo ospita.
#[derive(Default)]
pub struct FolderWatcher(Mutex<HashMap<String, Watch>>);

impl FolderWatcher {
    pub fn forget_window(&self, label: &str) {
        if let Ok(mut watches) = self.0.lock() {
            watches.retain(|_, watch| watch.window != label);
        }
    }
}

#[derive(Serialize, Clone)]
struct GitChanged {
    project: String,
    repo: String,
}

#[derive(Serialize, Clone)]
struct FsChanged {
    project: String,
    dirs: Vec<String>,
}

struct ChangeFilter {
    root: PathBuf,
    /// Il .gitignore della cartella e quello di ogni repository annidato, ognuno valido sotto la sua directory.
    gitignores: Vec<Gitignore>,
}

fn gitignore_of(dir: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(dir);
    builder.add(dir.join(".gitignore"));
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

impl ChangeFilter {
    fn new(root: &Path) -> Self {
        let mut dirs = vec![root.to_path_buf()];
        dirs.extend(find_repos(root).into_iter().filter(|repo| repo != root));
        Self { root: root.to_path_buf(), gitignores: dirs.iter().map(|dir| gitignore_of(dir)).collect() }
    }

    fn is_relevant(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else { return false };
        if relative.as_os_str().is_empty() {
            return false;
        }
        if relative.components().any(|part| ALWAYS_SKIPPED.iter().any(|skip| part.as_os_str() == *skip)) {
            return false;
        }
        let is_dir = path.is_dir();
        !self
            .gitignores
            .iter()
            .filter(|gitignore| path.starts_with(gitignore.path()))
            .any(|gitignore| gitignore.matched_path_or_any_parents(path, is_dir).is_ignore())
    }
}

/// Repository di `path` se il cambio è dentro un suo .git e altera branch, index o refs (esclusi oggetti, log e lock).
/// Il .git può stare a qualsiasi livello: una cartella può contenere più repository.
fn changed_repo(root: &Path, path: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(root).ok()?;
    let parts: Vec<_> = relative.components().collect();
    let git_at = parts.iter().position(|part| part.as_os_str() == ".git")?;
    let inside: PathBuf = parts[git_at + 1..].iter().collect();
    let is_lock = path.extension().is_some_and(|extension| extension == "lock");
    if is_lock || inside.starts_with("objects") || inside.starts_with("logs") || inside.starts_with("modules") {
        return None;
    }
    Some(parts[..git_at].iter().fold(root.to_path_buf(), |repo, part| repo.join(part)))
}

/// Directory il cui contenuto è cambiato: il frontend ricarica solo quelle già aperte.
fn changed_dirs(filter: &ChangeFilter, paths: impl Iterator<Item = PathBuf>) -> Vec<String> {
    paths
        .filter(|path| filter.is_relevant(path))
        .filter_map(|path| path.parent().map(Path::to_path_buf))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|dir| dir.to_string_lossy().into_owned())
        .collect()
}

#[tauri::command]
pub fn watch_folder(window: WebviewWindow, watcher: State<FolderWatcher>, project: String, path: Option<String>) -> Result<(), String> {
    let mut watches = watcher.0.lock().map_err(|e| e.to_string())?;
    watches.remove(&project);
    let Some(path) = path else { return Ok(()) };

    let root = PathBuf::from(&path);
    let filter = ChangeFilter::new(&root);
    let label = window.label().to_owned();
    let target = label.clone();
    let id = project.clone();
    let mut debouncer = new_debouncer(DEBOUNCE, move |result: DebounceEventResult| {
        let Ok(events) = result else { return };
        let repos: BTreeSet<PathBuf> = events.iter().filter_map(|event| changed_repo(&filter.root, &event.path)).collect();
        for repo in repos {
            let _ = window.emit_to(target.as_str(), "git-changed", GitChanged { project: id.clone(), repo: repo.to_string_lossy().into_owned() });
        }
        let dirs = changed_dirs(&filter, events.into_iter().map(|event| event.path));
        if !dirs.is_empty() {
            let _ = window.emit_to(target.as_str(), "fs-changed", FsChanged { project: id.clone(), dirs });
        }
    })
    .map_err(|e| e.to_string())?;
    debouncer.watcher().watch(&root, RecursiveMode::Recursive).map_err(|e| e.to_string())?;
    watches.insert(project, Watch { window: label, _debouncer: debouncer });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_git_internals_dependencies_and_ignored_paths() {
        let root = std::env::temp_dir().join("steroitor-watcher-test");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();
        let filter = ChangeFilter::new(&root);

        assert!(filter.is_relevant(&root.join("src").join("main.rs")));
        assert!(!filter.is_relevant(&root.join(".git").join("index")));
        assert!(!filter.is_relevant(&root.join("web").join("node_modules").join("x.js")));
        assert!(!filter.is_relevant(&root.join("target").join("debug").join("app.exe")));
        assert!(!filter.is_relevant(&root.join("build.log")));
        assert!(!filter.is_relevant(Path::new("C:\\elsewhere\\file.txt")));
    }

    #[test]
    fn applies_the_gitignore_of_each_nested_repository_only_inside_it() {
        let root = std::env::temp_dir().join("steroitor-watcher-nested");
        let fe = root.join("gipso-fe");
        std::fs::create_dir_all(fe.join(".git")).unwrap();
        std::fs::write(fe.join(".gitignore"), "dist/
").unwrap();
        let filter = ChangeFilter::new(&root);

        assert!(!filter.is_relevant(&fe.join("dist").join("main.js")));
        assert!(filter.is_relevant(&fe.join("src").join("main.ts")));
        assert!(filter.is_relevant(&root.join("dist").join("notes.txt")));
    }

    #[test]
    fn detects_relevant_git_metadata_changes() {
        let root = Path::new("C:\\repo");
        let git = root.join(".git");
        assert_eq!(changed_repo(root, &git.join("HEAD")), Some(root.to_path_buf()));
        assert_eq!(changed_repo(root, &git.join("index")), Some(root.to_path_buf()));
        assert_eq!(changed_repo(root, &git.join("refs").join("heads").join("main")), Some(root.to_path_buf()));
        assert_eq!(changed_repo(root, &git.join("index.lock")), None);
        assert_eq!(changed_repo(root, &git.join("objects").join("ab").join("cdef")), None);
        assert_eq!(changed_repo(root, &root.join("src").join("HEAD")), None);
    }

    #[test]
    fn tells_apart_nested_repositories() {
        let root = Path::new("C:\\gipso");
        let fe = root.join("gipso-fe");
        let be = root.join("gipso-be");
        assert_eq!(changed_repo(root, &fe.join(".git").join("HEAD")), Some(fe));
        assert_eq!(changed_repo(root, &be.join(".git").join("refs").join("heads").join("dev")), Some(be.clone()));
        assert_eq!(changed_repo(root, &be.join("src").join("main.ts")), None);
    }

    #[test]
    fn reports_each_parent_directory_once() {
        let root = std::env::temp_dir().join("steroitor-watcher-dedupe");
        let filter = ChangeFilter::new(&root);
        let src = root.join("src");
        let dirs = changed_dirs(&filter, [src.join("a.rs"), src.join("b.rs"), root.join("README.md")].into_iter());
        assert_eq!(dirs, vec![root.to_string_lossy().into_owned(), src.to_string_lossy().into_owned()]);
    }
}

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

#[derive(Default)]
pub struct FolderWatcher(Mutex<HashMap<String, Debouncer<RecommendedWatcher>>>);

impl FolderWatcher {
    pub fn forget(&self, label: &str) {
        if let Ok(mut watchers) = self.0.lock() {
            watchers.remove(label);
        }
    }
}

struct ChangeFilter {
    root: PathBuf,
    gitignore: Gitignore,
}

impl ChangeFilter {
    fn new(root: &Path) -> Self {
        let mut builder = GitignoreBuilder::new(root);
        builder.add(root.join(".gitignore"));
        Self { root: root.to_path_buf(), gitignore: builder.build().unwrap_or_else(|_| Gitignore::empty()) }
    }

    fn is_relevant(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else { return false };
        if relative.as_os_str().is_empty() {
            return false;
        }
        if relative.components().any(|part| ALWAYS_SKIPPED.iter().any(|skip| part.as_os_str() == *skip)) {
            return false;
        }
        !self.gitignore.matched_path_or_any_parents(path, path.is_dir()).is_ignore()
    }
}

/// Cambi dentro .git che alterano branch, index o refs (esclusi oggetti e lock).
fn touches_git_metadata(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root.join(".git")) else { return false };
    let is_lock = path.extension().is_some_and(|extension| extension == "lock");
    !is_lock && !relative.starts_with("objects") && !relative.starts_with("logs")
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
pub fn watch_folder(window: WebviewWindow, watcher: State<FolderWatcher>, path: Option<String>) -> Result<(), String> {
    let label = window.label().to_owned();
    let mut watchers = watcher.0.lock().map_err(|e| e.to_string())?;
    watchers.remove(&label);
    let Some(path) = path else { return Ok(()) };

    let root = PathBuf::from(&path);
    let filter = ChangeFilter::new(&root);
    let target = label.clone();
    let mut debouncer = new_debouncer(DEBOUNCE, move |result: DebounceEventResult| {
        let Ok(events) = result else { return };
        if events.iter().any(|event| touches_git_metadata(&filter.root, &event.path)) {
            let _ = window.emit_to(target.as_str(), "git-changed", ());
        }
        let dirs = changed_dirs(&filter, events.into_iter().map(|event| event.path));
        if !dirs.is_empty() {
            let _ = window.emit_to(target.as_str(), "fs-changed", dirs);
        }
    })
    .map_err(|e| e.to_string())?;
    debouncer.watcher().watch(&root, RecursiveMode::Recursive).map_err(|e| e.to_string())?;
    watchers.insert(label, debouncer);
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
    fn detects_relevant_git_metadata_changes() {
        let root = Path::new("C:\\repo");
        let git = root.join(".git");
        assert!(touches_git_metadata(root, &git.join("HEAD")));
        assert!(touches_git_metadata(root, &git.join("index")));
        assert!(touches_git_metadata(root, &git.join("refs").join("heads").join("main")));
        assert!(!touches_git_metadata(root, &git.join("index.lock")));
        assert!(!touches_git_metadata(root, &git.join("objects").join("ab").join("cdef")));
        assert!(!touches_git_metadata(root, &root.join("src").join("HEAD")));
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

use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, State, WebviewWindow};

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

#[tauri::command]
pub fn watch_folder(window: WebviewWindow, watcher: State<FolderWatcher>, project: String, path: Option<String>) -> Result<(), String> {
    let mut watches = watcher.0.lock().map_err(|e| e.to_string())?;
    watches.remove(&project);
    let Some(path) = path else { return Ok(()) };

    let label = window.label().to_owned();
    let target = label.clone();
    let id = project.clone();
    let debouncer = steroitor_core::watch::watch(&PathBuf::from(&path), move |repos, dirs| {
        for repo in repos {
            let _ = window.emit_to(target.as_str(), "git-changed", GitChanged { project: id.clone(), repo: repo.to_string_lossy().into_owned() });
        }
        if !dirs.is_empty() {
            let _ = window.emit_to(target.as_str(), "fs-changed", FsChanged { project: id.clone(), dirs });
        }
    })?;
    watches.insert(project, Watch { window: label, _debouncer: debouncer });
    Ok(())
}

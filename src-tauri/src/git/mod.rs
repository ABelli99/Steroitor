//! Comandi Tauri sopra il backend Git di `steroitor-core`.
pub mod commands;
pub mod discover;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use steroitor_core::git::runner::Git;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

const SETTINGS_FILE: &str = "settings.json";

/// Percorso dell'eseguibile git: `gitPath` in settings.json, altrimenti `git` dal PATH.
pub struct GitConfig {
    executable: RwLock<String>,
    next_id: AtomicU64,
}

impl GitConfig {
    pub fn load(app: &AppHandle) -> Self {
        let configured = app
            .path()
            .app_data_dir()
            .ok()
            .and_then(|dir| std::fs::read_to_string(dir.join(SETTINGS_FILE)).ok())
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|settings| settings.get("gitPath")?.as_str().map(str::to_owned));
        Self { executable: RwLock::new(configured.unwrap_or_else(|| "git".into())), next_id: AtomicU64::new(1) }
    }
}

/// I comandi finiscono nella Console della sola finestra che li ha lanciati.
pub fn git_for(window: &WebviewWindow) -> Git {
    let app = window.app_handle().clone();
    let config = app.state::<GitConfig>();
    let executable = config.executable.read().map(|value| value.clone()).unwrap_or_else(|_| "git".into());
    let label = window.label().to_owned();
    Git::new(
        executable,
        Box::new(move |mut log| {
            log.id = app.state::<GitConfig>().next_id.fetch_add(1, Ordering::Relaxed);
            let _ = app.emit_to(label.as_str(), "git-command", log);
        }),
    )
}

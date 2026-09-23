use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

static STARTED_AT: OnceLock<Instant> = OnceLock::new();

pub fn mark_start() {
    STARTED_AT.get_or_init(Instant::now);
}

pub fn resolve_paths(args: impl IntoIterator<Item = String>, cwd: &Path) -> Vec<String> {
    args.into_iter()
        .filter(|arg| !arg.starts_with('-'))
        .map(|arg| {
            let path = PathBuf::from(arg);
            let absolute = if path.is_absolute() { path } else { cwd.join(path) };
            absolute.to_string_lossy().into_owned()
        })
        .collect()
}

#[tauri::command]
pub fn startup_files() -> Vec<String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    resolve_paths(std::env::args().skip(1), &cwd)
}

/// Chiamato dal frontend quando l'editor è usabile. Con STEROITOR_MEASURE_FILE
/// impostata scrive lì i millisecondi dall'avvio (usato da scripts/measure.ps1).
#[tauri::command]
pub fn app_ready() -> u128 {
    let elapsed = STARTED_AT.get().map_or(0, |start| start.elapsed().as_millis());
    if let Ok(target) = std::env::var("STEROITOR_MEASURE_FILE") {
        let _ = std::fs::write(target, elapsed.to_string());
    }
    elapsed
}

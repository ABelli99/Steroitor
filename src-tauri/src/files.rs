pub use steroitor_core::text::{decode, TextFile};
use steroitor_core::text::encode;
use serde::Serialize;

#[tauri::command]
pub async fn read_text_file(path: String) -> Result<TextFile, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("Impossibile leggere {path}: {e}"))?;
    Ok(decode(&bytes))
}

#[tauri::command]
pub async fn write_text_file(path: String, content: String, encoding: String, bom: bool) -> Result<(), String> {
    let bytes = encode(&content, &encoding, bom)?;
    std::fs::write(&path, bytes).map_err(|e| format!("Impossibile salvare {path}: {e}"))
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Probe {
    File { content: String },
    Dir { dirs: Vec<String> },
    Missing,
}

const MAX_PROBE_BYTES: u64 = 8 * 1024 * 1024;

fn probe(path: &str) -> Probe {
    let Ok(meta) = std::fs::metadata(path) else { return Probe::Missing };
    if meta.is_dir() {
        // `path().is_dir()` segue i link simbolici: node_modules di pnpm è fatto di link.
        let dirs = std::fs::read_dir(path)
            .map(|entries| {
                entries.flatten().filter(|entry| entry.path().is_dir()).map(|entry| entry.file_name().to_string_lossy().into_owned()).collect()
            })
            .unwrap_or_default();
        return Probe::Dir { dirs };
    }
    if meta.len() > MAX_PROBE_BYTES {
        return Probe::Missing;
    }
    std::fs::read(path).map(|bytes| Probe::File { content: decode(&bytes).content }).unwrap_or(Probe::Missing)
}

/// Il type-check chiede file e cartelle a lotti: una chiamata per ogni livello di import da risolvere.
#[tauri::command]
pub async fn probe_paths(paths: Vec<String>) -> Vec<Probe> {
    paths.iter().map(|path| probe(path)).collect()
}

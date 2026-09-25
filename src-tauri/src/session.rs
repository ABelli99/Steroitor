use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const LEGACY_SESSION_FILE: &str = "session.json";
const PROJECT_SESSIONS_DIR: &str = "sessions";

/// Una sessione per cartella; il progetto senza cartella usa il vecchio session.json.
fn session_path(app: &AppHandle, folder: Option<&str>) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let path = match folder {
        None => dir.join(LEGACY_SESSION_FILE),
        Some(folder) => dir.join(PROJECT_SESSIONS_DIR).join(format!("{:016x}.json", fnv1a(&normalize(folder)))),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

fn normalize(folder: &str) -> String {
    folder.trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase()
}

/// Hash stabile tra versioni di Rust (a differenza di DefaultHasher), usato come nome file.
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

#[tauri::command]
pub async fn load_session(app: AppHandle, folder: Option<String>) -> Result<Option<String>, String> {
    let path = session_path(&app, folder.as_deref())?;
    if !path.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(path).map(Some).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_session(app: AppHandle, folder: Option<String>, data: String) -> Result<(), String> {
    let path = session_path(&app, folder.as_deref())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_folders_deterministically() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_ne!(fnv1a("c:\\repo"), fnv1a("c:\\other"));
    }

    #[test]
    fn the_same_folder_written_differently_shares_the_session() {
        assert_eq!(normalize("C:/Repo/"), normalize("c:\\repo"));
    }
}

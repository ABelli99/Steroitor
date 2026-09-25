use crate::windows::{self, Projects};
use std::path::PathBuf;
use tauri::{Manager, State, WebviewWindow};

const SESSION_FILE: &str = "session.json";
const PROJECT_SESSIONS_DIR: &str = "sessions";

/// La finestra principale ha la sessione di sempre; le altre una per cartella, così riaprendo
/// lo stesso progetto in una nuova finestra ritornano le sue tab.
fn session_path(window: &WebviewWindow, projects: &Projects) -> Result<Option<PathBuf>, String> {
    let dir = window.app_handle().path().app_data_dir().map_err(|e| e.to_string())?;
    let path = if window.label() == windows::MAIN {
        dir.join(SESSION_FILE)
    } else {
        let Some(folder) = projects.folder_of(window.label()) else { return Ok(None) };
        dir.join(PROJECT_SESSIONS_DIR).join(format!("{:016x}.json", fnv1a(&folder.to_lowercase())))
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(Some(path))
}

/// Hash stabile tra versioni di Rust (a differenza di DefaultHasher), usato come nome file.
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

#[tauri::command]
pub async fn load_session(window: WebviewWindow, projects: State<'_, Projects>) -> Result<Option<String>, String> {
    let Some(path) = session_path(&window, &projects)? else { return Ok(None) };
    if !path.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(path).map(Some).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_session(window: WebviewWindow, projects: State<'_, Projects>, data: String) -> Result<(), String> {
    let Some(path) = session_path(&window, &projects)? else { return Ok(()) };
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
        assert_eq!(fnv1a("c:\\repo"), fnv1a("c:\\repo"));
        assert_ne!(fnv1a("c:\\repo"), fnv1a("c:\\other"));
    }
}

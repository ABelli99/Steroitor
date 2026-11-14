use steroitor_core::fs::{self, Entry, LineMatch, PathState};

#[tauri::command]
pub async fn list_dir(path: String) -> Result<Vec<Entry>, String> {
    fs::list_dir(path)
}

#[tauri::command]
pub async fn list_files(root: String) -> Vec<String> {
    fs::list_files(root)
}

#[tauri::command]
pub async fn search_files(root: String, needle: String, extensions: Vec<String>) -> Vec<LineMatch> {
    fs::search_files(root, needle, extensions)
}

#[tauri::command]
pub async fn create_file(parent: String, name: String) -> Result<String, String> {
    fs::create_file(parent, name)
}

#[tauri::command]
pub async fn create_dir(parent: String, name: String) -> Result<String, String> {
    fs::create_dir(parent, name)
}

#[tauri::command]
pub async fn rename_path(path: String, new_name: String) -> Result<String, String> {
    fs::rename_path(path, new_name)
}

#[tauri::command]
pub async fn delete_to_trash(path: String) -> Result<(), String> {
    fs::delete_to_trash(path)
}

#[tauri::command]
pub fn path_state(path: String) -> PathState {
    fs::path_state(path)
}

#[tauri::command]
pub fn reveal_in_os(path: String) -> Result<(), String> {
    fs::reveal_in_os(path)
}

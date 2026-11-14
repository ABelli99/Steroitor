use std::path::Path;

pub use steroitor_core::repos::find_repos;

#[tauri::command]
pub async fn git_find_repos(root: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        find_repos(Path::new(&root)).into_iter().map(|path| path.to_string_lossy().into_owned()).collect()
    })
    .await
    .map_err(|e| e.to_string())
}

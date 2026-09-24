use tauri::AppHandle;

use super::parse::{Branch, Commit, RepoStatus};
use super::runner::Git;
use crate::files::TextFile;

/// Esegue un'operazione git fuori dal thread dei comandi Tauri.
async fn with_git<T: Send + 'static>(app: AppHandle, task: impl FnOnce(Git) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || task(Git::from_app(&app))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn git_version(app: AppHandle) -> Result<String, String> {
    with_git(app, |git| git.version()).await
}

#[tauri::command]
pub async fn git_repo_root(app: AppHandle, path: String) -> Result<Option<String>, String> {
    with_git(app, move |git| git.repo_root(&path)).await
}

#[tauri::command]
pub async fn git_status(app: AppHandle, root: String) -> Result<RepoStatus, String> {
    with_git(app, move |git| git.status(&root)).await
}

#[tauri::command]
pub async fn git_branches(app: AppHandle, root: String) -> Result<Vec<Branch>, String> {
    with_git(app, move |git| git.branches(&root)).await
}

#[tauri::command]
pub async fn git_log(app: AppHandle, root: String, reference: Option<String>, skip: u32, limit: u32) -> Result<Vec<Commit>, String> {
    with_git(app, move |git| git.log(&root, reference.as_deref(), skip, limit)).await
}

#[tauri::command]
pub async fn git_head_content(app: AppHandle, root: String, path: String) -> Result<Option<TextFile>, String> {
    with_git(app, move |git| git.head_content(&root, &path)).await
}

#[tauri::command]
pub async fn git_show_commit(app: AppHandle, root: String, hash: String) -> Result<String, String> {
    with_git(app, move |git| git.show_commit(&root, &hash)).await
}

#[tauri::command]
pub async fn git_stage(app: AppHandle, root: String, paths: Vec<String>) -> Result<(), String> {
    with_git(app, move |git| git.stage(&root, &paths)).await
}

#[tauri::command]
pub async fn git_unstage(app: AppHandle, root: String, paths: Vec<String>) -> Result<(), String> {
    with_git(app, move |git| git.unstage(&root, &paths)).await
}

#[tauri::command]
pub async fn git_commit(app: AppHandle, root: String, message: String) -> Result<(), String> {
    with_git(app, move |git| git.commit(&root, &message)).await
}

#[tauri::command]
pub async fn git_push(app: AppHandle, root: String) -> Result<(), String> {
    with_git(app, move |git| git.push(&root)).await
}

#[tauri::command]
pub async fn git_pull(app: AppHandle, root: String, rebase: bool) -> Result<(), String> {
    with_git(app, move |git| git.pull(&root, rebase)).await
}

#[tauri::command]
pub async fn git_fetch(app: AppHandle, root: String) -> Result<(), String> {
    with_git(app, move |git| git.fetch(&root)).await
}

#[tauri::command]
pub async fn git_switch(app: AppHandle, root: String, branch: String, remote: bool) -> Result<(), String> {
    with_git(app, move |git| git.switch(&root, &branch, remote)).await
}

#[tauri::command]
pub async fn git_create_branch(app: AppHandle, root: String, name: String, checkout: bool) -> Result<(), String> {
    with_git(app, move |git| git.create_branch(&root, &name, checkout)).await
}

#[tauri::command]
pub async fn git_staged_crlf_files(app: AppHandle, root: String) -> Result<Vec<String>, String> {
    with_git(app, move |git| git.staged_crlf_files(&root)).await
}

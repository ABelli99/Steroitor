use tauri::AppHandle;

use super::history::{ConflictVersions, LogFilter, ResetMode};
use super::parse::{BlameLine, Branch, Commit, RepoStatus, Stash};
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
pub async fn git_log(app: AppHandle, root: String, filter: LogFilter, skip: u32, limit: u32) -> Result<Vec<Commit>, String> {
    with_git(app, move |git| git.log_filtered(&root, &filter, skip, limit)).await
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
pub async fn git_create_branch(app: AppHandle, root: String, name: String, checkout: bool, start: Option<String>) -> Result<(), String> {
    with_git(app, move |git| git.create_branch(&root, &name, checkout, start.as_deref())).await
}

#[tauri::command]
pub async fn git_staged_crlf_files(app: AppHandle, root: String) -> Result<Vec<String>, String> {
    with_git(app, move |git| git.staged_crlf_files(&root)).await
}

#[tauri::command]
pub async fn git_continue_operation(app: AppHandle, root: String) -> Result<(), String> {
    with_git(app, move |git| git.continue_operation(&root)).await
}

#[tauri::command]
pub async fn git_abort_operation(app: AppHandle, root: String) -> Result<(), String> {
    with_git(app, move |git| git.abort_operation(&root)).await
}

#[tauri::command]
pub async fn git_checkout_commit(app: AppHandle, root: String, hash: String) -> Result<(), String> {
    with_git(app, move |git| git.checkout_commit(&root, &hash)).await
}

#[tauri::command]
pub async fn git_cherry_pick(app: AppHandle, root: String, hash: String) -> Result<(), String> {
    with_git(app, move |git| git.cherry_pick(&root, &hash)).await
}

#[tauri::command]
pub async fn git_revert(app: AppHandle, root: String, hash: String) -> Result<(), String> {
    with_git(app, move |git| git.revert(&root, &hash)).await
}

#[tauri::command]
pub async fn git_reset(app: AppHandle, root: String, hash: String, mode: ResetMode) -> Result<(), String> {
    with_git(app, move |git| git.reset(&root, &hash, mode)).await
}

#[tauri::command]
pub async fn git_create_tag(app: AppHandle, root: String, name: String, hash: String) -> Result<(), String> {
    with_git(app, move |git| git.create_tag(&root, &name, &hash)).await
}

#[tauri::command]
pub async fn git_merge(app: AppHandle, root: String, branch: String) -> Result<(), String> {
    with_git(app, move |git| git.merge(&root, &branch)).await
}

#[tauri::command]
pub async fn git_rebase(app: AppHandle, root: String, onto: String) -> Result<(), String> {
    with_git(app, move |git| git.rebase(&root, &onto)).await
}

#[tauri::command]
pub async fn git_stash_push(app: AppHandle, root: String, message: String) -> Result<(), String> {
    with_git(app, move |git| git.stash_push(&root, &message)).await
}

#[tauri::command]
pub async fn git_stash_pop(app: AppHandle, root: String, name: String) -> Result<(), String> {
    with_git(app, move |git| git.stash_pop(&root, &name)).await
}

#[tauri::command]
pub async fn git_stash_drop(app: AppHandle, root: String, name: String) -> Result<(), String> {
    with_git(app, move |git| git.stash_drop(&root, &name)).await
}

#[tauri::command]
pub async fn git_amend(app: AppHandle, root: String, message: String) -> Result<(), String> {
    with_git(app, move |git| git.amend(&root, &message)).await
}

#[tauri::command]
pub async fn git_blame(app: AppHandle, root: String, path: String) -> Result<Vec<BlameLine>, String> {
    with_git(app, move |git| git.blame(&root, &path)).await
}

#[tauri::command]
pub async fn git_last_commit_message(app: AppHandle, root: String) -> Result<String, String> {
    with_git(app, move |git| git.last_commit_message(&root)).await
}

#[tauri::command]
pub async fn git_head_is_pushed(app: AppHandle, root: String) -> Result<bool, String> {
    with_git(app, move |git| git.head_is_pushed(&root)).await
}

#[tauri::command]
pub async fn git_stashes(app: AppHandle, root: String) -> Result<Vec<Stash>, String> {
    with_git(app, move |git| git.stashes(&root)).await
}

#[tauri::command]
pub async fn git_conflict_versions(app: AppHandle, root: String, path: String) -> Result<ConflictVersions, String> {
    with_git(app, move |git| git.conflict_versions(&root, &path)).await
}

#[tauri::command]
pub async fn git_index_content(app: AppHandle, root: String, path: String) -> Result<Option<String>, String> {
    with_git(app, move |git| git.index_content(&root, &path)).await
}

#[tauri::command]
pub async fn git_stage_content(app: AppHandle, root: String, path: String, content: String) -> Result<(), String> {
    with_git(app, move |git| git.stage_content(&root, &path, &content)).await
}

#[tauri::command]
pub async fn git_clone(app: AppHandle, url: String, target: String) -> Result<(), String> {
    with_git(app, move |git| git.clone_repo(&url, &target)).await
}

#[tauri::command]
pub async fn git_init(app: AppHandle, path: String) -> Result<(), String> {
    with_git(app, move |git| git.init_repo(&path)).await
}

use tauri::WebviewWindow;

use super::changes::ChangedFile;
use super::history::{ConflictVersions, LogFilter, ResetMode};
use super::parse::{BlameLine, Branch, Commit, RepoStatus, Stash};
use super::runner::Git;
use crate::files::TextFile;

/// Esegue un'operazione git fuori dal thread dei comandi Tauri.
async fn with_git<T: Send + 'static>(window: WebviewWindow, task: impl FnOnce(Git) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || task(Git::for_window(&window))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn git_version(window: WebviewWindow) -> Result<String, String> {
    with_git(window, |git| git.version()).await
}

#[tauri::command]
pub async fn git_repo_root(window: WebviewWindow, path: String) -> Result<Option<String>, String> {
    with_git(window, move |git| git.repo_root(&path)).await
}

#[tauri::command]
pub async fn git_status(window: WebviewWindow, root: String) -> Result<RepoStatus, String> {
    with_git(window, move |git| git.status(&root)).await
}

#[tauri::command]
pub async fn git_branches(window: WebviewWindow, root: String) -> Result<Vec<Branch>, String> {
    with_git(window, move |git| git.branches(&root)).await
}

#[tauri::command]
pub async fn git_log(window: WebviewWindow, root: String, filter: LogFilter, skip: u32, limit: u32) -> Result<Vec<Commit>, String> {
    with_git(window, move |git| git.log_filtered(&root, &filter, skip, limit)).await
}

#[tauri::command]
pub async fn git_head_content(window: WebviewWindow, root: String, path: String) -> Result<Option<TextFile>, String> {
    with_git(window, move |git| git.head_content(&root, &path)).await
}


#[tauri::command]
pub async fn git_stage(window: WebviewWindow, root: String, paths: Vec<String>) -> Result<(), String> {
    with_git(window, move |git| git.stage(&root, &paths)).await
}

#[tauri::command]
pub async fn git_unstage(window: WebviewWindow, root: String, paths: Vec<String>) -> Result<(), String> {
    with_git(window, move |git| git.unstage(&root, &paths)).await
}

#[tauri::command]
pub async fn git_commit(window: WebviewWindow, root: String, message: String) -> Result<(), String> {
    with_git(window, move |git| git.commit(&root, &message)).await
}

#[tauri::command]
pub async fn git_push(window: WebviewWindow, root: String) -> Result<(), String> {
    with_git(window, move |git| git.push(&root)).await
}

#[tauri::command]
pub async fn git_pull(window: WebviewWindow, root: String, rebase: bool) -> Result<(), String> {
    with_git(window, move |git| git.pull(&root, rebase)).await
}

#[tauri::command]
pub async fn git_fetch(window: WebviewWindow, root: String) -> Result<(), String> {
    with_git(window, move |git| git.fetch(&root)).await
}

#[tauri::command]
pub async fn git_switch(window: WebviewWindow, root: String, branch: String, remote: bool) -> Result<(), String> {
    with_git(window, move |git| git.switch(&root, &branch, remote)).await
}

#[tauri::command]
pub async fn git_create_branch(window: WebviewWindow, root: String, name: String, checkout: bool, start: Option<String>) -> Result<(), String> {
    with_git(window, move |git| git.create_branch(&root, &name, checkout, start.as_deref())).await
}

#[tauri::command]
pub async fn git_staged_crlf_files(window: WebviewWindow, root: String) -> Result<Vec<String>, String> {
    with_git(window, move |git| git.staged_crlf_files(&root)).await
}

#[tauri::command]
pub async fn git_continue_operation(window: WebviewWindow, root: String) -> Result<(), String> {
    with_git(window, move |git| git.continue_operation(&root)).await
}

#[tauri::command]
pub async fn git_abort_operation(window: WebviewWindow, root: String) -> Result<(), String> {
    with_git(window, move |git| git.abort_operation(&root)).await
}

#[tauri::command]
pub async fn git_checkout_commit(window: WebviewWindow, root: String, hash: String) -> Result<(), String> {
    with_git(window, move |git| git.checkout_commit(&root, &hash)).await
}

#[tauri::command]
pub async fn git_cherry_pick(window: WebviewWindow, root: String, hash: String) -> Result<(), String> {
    with_git(window, move |git| git.cherry_pick(&root, &hash)).await
}

#[tauri::command]
pub async fn git_revert(window: WebviewWindow, root: String, hash: String) -> Result<(), String> {
    with_git(window, move |git| git.revert(&root, &hash)).await
}

#[tauri::command]
pub async fn git_reset(window: WebviewWindow, root: String, hash: String, mode: ResetMode) -> Result<(), String> {
    with_git(window, move |git| git.reset(&root, &hash, mode)).await
}

#[tauri::command]
pub async fn git_create_tag(window: WebviewWindow, root: String, name: String, hash: String) -> Result<(), String> {
    with_git(window, move |git| git.create_tag(&root, &name, &hash)).await
}

#[tauri::command]
pub async fn git_merge(window: WebviewWindow, root: String, branch: String) -> Result<(), String> {
    with_git(window, move |git| git.merge(&root, &branch)).await
}

#[tauri::command]
pub async fn git_rebase(window: WebviewWindow, root: String, onto: String) -> Result<(), String> {
    with_git(window, move |git| git.rebase(&root, &onto)).await
}

#[tauri::command]
pub async fn git_stash_push(window: WebviewWindow, root: String, message: String) -> Result<(), String> {
    with_git(window, move |git| git.stash_push(&root, &message)).await
}

#[tauri::command]
pub async fn git_stash_pop(window: WebviewWindow, root: String, name: String) -> Result<(), String> {
    with_git(window, move |git| git.stash_pop(&root, &name)).await
}

#[tauri::command]
pub async fn git_stash_drop(window: WebviewWindow, root: String, name: String) -> Result<(), String> {
    with_git(window, move |git| git.stash_drop(&root, &name)).await
}

#[tauri::command]
pub async fn git_amend(window: WebviewWindow, root: String, message: String) -> Result<(), String> {
    with_git(window, move |git| git.amend(&root, &message)).await
}

#[tauri::command]
pub async fn git_blame(window: WebviewWindow, root: String, path: String) -> Result<Vec<BlameLine>, String> {
    with_git(window, move |git| git.blame(&root, &path)).await
}

#[tauri::command]
pub async fn git_last_commit_message(window: WebviewWindow, root: String) -> Result<String, String> {
    with_git(window, move |git| git.last_commit_message(&root)).await
}

#[tauri::command]
pub async fn git_head_is_pushed(window: WebviewWindow, root: String) -> Result<bool, String> {
    with_git(window, move |git| git.head_is_pushed(&root)).await
}

#[tauri::command]
pub async fn git_stashes(window: WebviewWindow, root: String) -> Result<Vec<Stash>, String> {
    with_git(window, move |git| git.stashes(&root)).await
}

#[tauri::command]
pub async fn git_conflict_versions(window: WebviewWindow, root: String, path: String) -> Result<ConflictVersions, String> {
    with_git(window, move |git| git.conflict_versions(&root, &path)).await
}

#[tauri::command]
pub async fn git_index_content(window: WebviewWindow, root: String, path: String) -> Result<Option<String>, String> {
    with_git(window, move |git| git.index_content(&root, &path)).await
}

#[tauri::command]
pub async fn git_stage_content(window: WebviewWindow, root: String, path: String, content: String) -> Result<(), String> {
    with_git(window, move |git| git.stage_content(&root, &path, &content)).await
}

#[tauri::command]
pub async fn git_clone(window: WebviewWindow, url: String, target: String) -> Result<(), String> {
    with_git(window, move |git| git.clone_repo(&url, &target)).await
}

#[tauri::command]
pub async fn git_init(window: WebviewWindow, path: String) -> Result<(), String> {
    with_git(window, move |git| git.init_repo(&path)).await
}

#[tauri::command]
pub async fn git_reword(window: WebviewWindow, root: String, hash: String, message: String) -> Result<(), String> {
    with_git(window, move |git| git.reword(&root, &hash, &message)).await
}

#[tauri::command]
pub async fn git_commit_message(window: WebviewWindow, root: String, hash: String) -> Result<String, String> {
    with_git(window, move |git| git.commit_message(&root, &hash)).await
}

#[tauri::command]
pub async fn git_commit_changes(window: WebviewWindow, root: String, hash: String) -> Result<Vec<ChangedFile>, String> {
    with_git(window, move |git| git.commit_changes(&root, &hash)).await
}

#[tauri::command]
pub async fn git_file_at(window: WebviewWindow, root: String, revision: String, path: String) -> Result<Option<String>, String> {
    with_git(window, move |git| git.file_at(&root, &revision, &path)).await
}

mod explorer;
mod files;
mod git;
mod session;
mod shells;
mod startup;
mod terminal;
mod updates;
mod watcher;
mod windows;

use std::path::Path;
use tauri::{Emitter, Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    startup::mark_start();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let files = startup::resolve_paths(argv.into_iter().skip(1), Path::new(&cwd));
            let Some(window) = windows::primary(app) else { return };
            let _ = window.unminimize();
            let _ = window.set_focus();
            let _ = window.emit_to(window.label(), "open-files", files);
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(watcher::FolderWatcher::default())
        .manage(terminal::Terminals::default())
        .on_window_event(|window, event| {
            if !matches!(event, WindowEvent::Destroyed) {
                return;
            }
            let app = window.app_handle();
            let label = window.label();
            let windows = app.state::<windows::Windows>();
            windows.window_destroyed(label);
            windows.persist();
            app.state::<watcher::FolderWatcher>().forget_window(label);
            app.state::<terminal::Terminals>().close_window(label);
        })
        .setup(|app| {
            app.manage(git::GitConfig::load(app.handle()));
            app.manage(windows::Windows::load(app.handle()));
            windows::restore_extra_windows(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            explorer::list_dir,
            explorer::list_files,
            explorer::create_file,
            explorer::create_dir,
            explorer::rename_path,
            explorer::delete_to_trash,
            explorer::reveal_in_os,
            explorer::path_state,
            files::read_text_file,
            files::write_text_file,
            git::commands::git_version,
            git::commands::git_repo_root,
            git::discover::git_find_repos,
            git::commands::git_status,
            git::commands::git_branches,
            git::commands::git_log,
            git::commands::git_head_content,
            git::commands::git_stage,
            git::commands::git_unstage,
            git::commands::git_commit,
            git::commands::git_push,
            git::commands::git_pull,
            git::commands::git_fetch,
            git::commands::git_switch,
            git::commands::git_create_branch,
            git::commands::git_staged_crlf_files,
            git::commands::git_continue_operation,
            git::commands::git_abort_operation,
            git::commands::git_checkout_commit,
            git::commands::git_cherry_pick,
            git::commands::git_revert,
            git::commands::git_reset,
            git::commands::git_create_tag,
            git::commands::git_merge,
            git::commands::git_rebase,
            git::commands::git_stash_push,
            git::commands::git_stash_pop,
            git::commands::git_stash_drop,
            git::commands::git_amend,
            git::commands::git_blame,
            git::commands::git_last_commit_message,
            git::commands::git_head_is_pushed,
            git::commands::git_stashes,
            git::commands::git_conflict_versions,
            git::commands::git_index_content,
            git::commands::git_stage_content,
            git::commands::git_clone,
            git::commands::git_init,
            git::commands::git_reword,
            git::commands::git_commit_changes,
            git::commands::git_file_at,
            git::commands::git_commit_message,
            session::load_session,
            session::save_session,
            startup::startup_files,
            startup::app_ready,
            watcher::watch_folder,
            shells::terminal_shells,
            terminal::terminal_open,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            updates::open_release_page,
            windows::window_state,
            windows::set_window_state,
            windows::locate_project,
            windows::activate_project,
            windows::detach_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

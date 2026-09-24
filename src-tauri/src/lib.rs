mod explorer;
mod files;
mod git;
mod session;
mod startup;
mod watcher;

use std::path::Path;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    startup::mark_start();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let files = startup::resolve_paths(argv.into_iter().skip(1), Path::new(&cwd));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            let _ = app.emit("open-files", files);
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(watcher::FolderWatcher::default())
        .setup(|app| {
            app.manage(git::GitConfig::load(app.handle()));
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
            files::read_text_file,
            files::write_text_file,
            git::git_version,
            git::git_repo_root,
            git::git_status,
            git::git_branches,
            git::git_log,
            git::git_head_content,
            session::load_session,
            session::save_session,
            startup::startup_files,
            startup::app_ready,
            watcher::watch_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod files;
mod session;
mod startup;

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
        .invoke_handler(tauri::generate_handler![
            files::read_text_file,
            files::write_text_file,
            session::load_session,
            session::save_session,
            startup::startup_files,
            startup::app_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

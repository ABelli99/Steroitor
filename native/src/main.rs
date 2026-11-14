#![windows_subsystem = "windows"]

mod app;
mod clone_dialog;
mod compare;
mod definition_link;
mod dialogs;
mod diff;
mod explorer;
mod find;
mod git;
mod hunks;
mod icons;
mod lang;
mod list;
mod minimap;
mod paint;
mod panel;
mod plugins;
mod prompt;
mod quick_open;
mod sci;
mod side_by_side;
mod session;
mod settings_page;
mod terminal;
mod theme;
mod vt;

use std::path::PathBuf;

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage, MSG};

extern "C" {
    fn Scintilla_RegisterClasses(instance: *mut std::ffi::c_void) -> i32;
}

/// Senza console un panic sparirebbe senza traccia: lo si scrive in `%APPDATA%\it.overzoom.steroitor\native\crash.log`.
fn log_panics() {
    std::panic::set_hook(Box::new(|info| {
        let Some(dir) = std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("it.overzoom.steroitor").join("native")) else { return };
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("crash.log"), format!("{info}\n\n{}", std::backtrace::Backtrace::force_capture()));
    }));
}

fn main() {
    log_panics();
    let mut files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    // Istanza singola: file e cartelle vanno alla finestra già aperta. `--new-instance` ne apre comunque una nuova.
    let before = files.len();
    files.retain(|file| file.as_os_str() != "--new-instance");
    if files.len() == before && app::forward_to_running(&files) {
        return;
    }
    // SAFETY: inizializzazione Win32 sul thread principale, prima di creare finestre.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(std::ptr::null());
        Scintilla_RegisterClasses(instance);
        let app = app::App::create(instance, files);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if app.shortcut(&msg) {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

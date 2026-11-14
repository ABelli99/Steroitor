#![windows_subsystem = "windows"]

mod app;
mod lang;
mod sci;
mod theme;

use std::path::PathBuf;

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage, MSG};

extern "C" {
    fn Scintilla_RegisterClasses(instance: *mut std::ffi::c_void) -> i32;
}

fn main() {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
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

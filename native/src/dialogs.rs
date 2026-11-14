//! Dialoghi di sistema (apri, salva, scegli cartella, messaggi) e appunti.
use std::ffi::c_void;
use std::path::{Path, PathBuf};

use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{GlobalFree, HWND};
use windows_sys::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE};

const CF_UNICODETEXT: u32 = 13;

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn message_box(owner: HWND, text: &str, flags: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    let text = wide(text);
    let title = wide("Steroitor");
    // SAFETY: stringhe terminate da zero che vivono per tutta la chiamata.
    unsafe { MessageBoxW(owner, text.as_ptr(), title.as_ptr(), flags) }
}

fn file_dialog(owner: HWND, suggested: &str, save: bool) -> Option<Vec<u16>> {
    let mut buffer = vec![0u16; 32 * 1024];
    for (slot, unit) in buffer.iter_mut().zip(suggested.encode_utf16().take(1000)) {
        *slot = unit;
    }
    let filter: Vec<u16> = "Tutti i file (*.*)\0*.*\0\0".encode_utf16().collect();
    // SAFETY: buffer e filtro vivono per tutta la chiamata; nMaxFile ne riporta la lunghezza.
    let accepted = unsafe {
        let mut dialog: OPENFILENAMEW = std::mem::zeroed();
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.hwndOwner = owner;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = buffer.as_mut_ptr();
        dialog.nMaxFile = buffer.len() as u32;
        if save {
            dialog.Flags = OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;
            GetSaveFileNameW(&mut dialog) != 0
        } else {
            dialog.Flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_ALLOWMULTISELECT | OFN_NOCHANGEDIR;
            GetOpenFileNameW(&mut dialog) != 0
        }
    };
    accepted.then_some(buffer)
}

pub fn open_files(owner: HWND) -> Vec<PathBuf> {
    let Some(buffer) = file_dialog(owner, "", false) else { return Vec::new() };
    // Una selezione: il percorso completo. Più file: la cartella, poi i nomi, separati da \0.
    let parts: Vec<String> = buffer.split(|&unit| unit == 0).take_while(|part| !part.is_empty()).map(String::from_utf16_lossy).collect();
    match parts.as_slice() {
        [single] => vec![PathBuf::from(single)],
        [dir, names @ ..] => names.iter().map(|name| Path::new(dir).join(name)).collect(),
        [] => Vec::new(),
    }
}

pub fn save_file(owner: HWND, suggested: &str) -> Option<PathBuf> {
    let buffer = file_dialog(owner, suggested, true)?;
    Some(PathBuf::from(String::from_utf16_lossy(buffer.split(|&unit| unit == 0).next().unwrap_or(&[]))))
}

const CLSID_FILE_OPEN_DIALOG: GUID = GUID::from_u128(0xdc1c5a9c_e88a_4dde_a5a1_60f82a20aef7);
const IID_IFILE_OPEN_DIALOG: GUID = GUID::from_u128(0xd57c7288_d4ad_4768_be02_9d969532d960);
const FOS_PICKFOLDERS: u32 = 0x20;
const FOS_FORCEFILESYSTEM: u32 = 0x40;
const SIGDN_FILESYSPATH: i32 = 0x8005_8000_u32 as i32;

/// Posizioni nella vtable di IFileOpenDialog e IShellItem (ordine fisso dell'ABI COM).
const SHOW: usize = 3;
const SET_OPTIONS: usize = 9;
const GET_RESULT: usize = 20;
const RELEASE: usize = 2;
const GET_DISPLAY_NAME: usize = 5;

/// Metodo `index` della vtable dell'oggetto COM `object`.
unsafe fn method<T>(object: *mut c_void, index: usize) -> T {
    let vtable = *(object as *const *const usize);
    std::mem::transmute_copy(&*vtable.add(index))
}

/// Il dialogo "scegli cartella" moderno di Windows (IFileOpenDialog con FOS_PICKFOLDERS).
pub fn pick_folder(owner: HWND) -> Option<PathBuf> {
    // SAFETY: chiamate COM secondo l'ABI di IFileOpenDialog/IShellItem; ogni riferimento ottenuto viene rilasciato.
    unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let mut dialog: *mut c_void = std::ptr::null_mut();
        if CoCreateInstance(&CLSID_FILE_OPEN_DIALOG, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, &IID_IFILE_OPEN_DIALOG, &mut dialog) != 0 {
            return None;
        }
        let release: unsafe extern "system" fn(*mut c_void) -> u32 = method(dialog, RELEASE);
        let set_options: unsafe extern "system" fn(*mut c_void, u32) -> i32 = method(dialog, SET_OPTIONS);
        let show: unsafe extern "system" fn(*mut c_void, HWND) -> i32 = method(dialog, SHOW);
        let get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 = method(dialog, GET_RESULT);
        set_options(dialog, FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);
        let mut folder = None;
        let mut item: *mut c_void = std::ptr::null_mut();
        if show(dialog, owner) == 0 && get_result(dialog, &mut item) == 0 {
            let display_name: unsafe extern "system" fn(*mut c_void, i32, *mut *mut u16) -> i32 = method(item, GET_DISPLAY_NAME);
            let mut name: *mut u16 = std::ptr::null_mut();
            if display_name(item, SIGDN_FILESYSPATH, &mut name) == 0 {
                let length = (0..).take_while(|&index| *name.add(index) != 0).count();
                folder = Some(PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(name, length))));
                CoTaskMemFree(name.cast());
            }
            let release_item: unsafe extern "system" fn(*mut c_void) -> u32 = method(item, RELEASE);
            release_item(item);
        }
        release(dialog);
        folder
    }
}

pub fn copy_to_clipboard(owner: HWND, text: &str) {
    let units = wide(text);
    let bytes = units.len() * 2;
    // SAFETY: la memoria globale passa agli appunti con SetClipboardData; la si libera solo se il passaggio fallisce.
    unsafe {
        if OpenClipboard(owner) == 0 {
            return;
        }
        EmptyClipboard();
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes);
        let target = GlobalLock(memory) as *mut u16;
        if !target.is_null() {
            std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
            GlobalUnlock(memory);
            if SetClipboardData(CF_UNICODETEXT, memory).is_null() {
                GlobalFree(memory);
            }
        }
        CloseClipboard();
    }
}

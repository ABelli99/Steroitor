use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

const MAX_INDEXED_FILES: usize = 100_000;
const FORBIDDEN_NAME_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    name: String,
    path: String,
    is_dir: bool,
}

fn is_dir_entry(entry: &fs::DirEntry) -> bool {
    entry
        .file_type()
        .map(|kind| kind.is_dir() || (kind.is_symlink() && entry.path().is_dir()))
        .unwrap_or(false)
}

fn validate_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains(FORBIDDEN_NAME_CHARS) {
        return Err(format!("Nome non valido: \"{name}\""));
    }
    Ok(name)
}

fn child_path(parent: &str, name: &str) -> Result<PathBuf, String> {
    let path = Path::new(parent).join(validate_name(name)?);
    if path.exists() {
        return Err(format!("Esiste già: {}", path.display()));
    }
    Ok(path)
}

fn display(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn list_dir(path: String) -> Result<Vec<Entry>, String> {
    let mut entries: Vec<Entry> = fs::read_dir(&path)
        .map_err(|e| format!("Impossibile leggere {path}: {e}"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() != ".git")
        .map(|entry| Entry {
            is_dir: is_dir_entry(&entry),
            name: entry.file_name().to_string_lossy().into_owned(),
            path: display(entry.path()),
        })
        .collect();
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

/// Tutti i file del progetto rispettando .gitignore, per Quick Open.
#[tauri::command]
pub async fn list_files(root: String) -> Vec<String> {
    ignore::WalkBuilder::new(&root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .take(MAX_INDEXED_FILES)
        .map(|entry| display(entry.into_path()))
        .collect()
}

#[tauri::command]
pub async fn create_file(parent: String, name: String) -> Result<String, String> {
    let path = child_path(&parent, &name)?;
    OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| e.to_string())?;
    Ok(display(path))
}

#[tauri::command]
pub async fn create_dir(parent: String, name: String) -> Result<String, String> {
    let path = child_path(&parent, &name)?;
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    Ok(display(path))
}

#[tauri::command]
pub async fn rename_path(path: String, new_name: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    let parent = source.parent().ok_or("Impossibile rinominare la radice")?;
    let target = parent.join(validate_name(&new_name)?);
    let only_case_changes = target.to_string_lossy().to_lowercase() == path.to_lowercase();
    if target.exists() && !only_case_changes {
        return Err(format!("Esiste già: {}", target.display()));
    }
    fs::rename(&source, &target).map_err(|e| e.to_string())?;
    Ok(display(target))
}

#[tauri::command]
pub async fn delete_to_trash(path: String) -> Result<(), String> {
    trash::delete(&path).map_err(|e| format!("Impossibile eliminare {path}: {e}"))
}

#[tauri::command]
pub fn reveal_in_os(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("explorer");
        command.raw_arg(format!("/select,\"{path}\""));
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.args(["-R", &path]);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(Path::new(&path).parent().unwrap_or(Path::new("/")));
        command
    };
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_names_that_escape_the_parent() {
        for name in ["", " ", ".", "..", "a/b", "..\\x", "c:x", "a?b"] {
            assert!(validate_name(name).is_err(), "{name:?} dovrebbe essere rifiutato");
        }
        assert_eq!(validate_name("  note.txt ").unwrap(), "note.txt");
    }
}

//! Sessione (cartella, tab aperte, layout) e impostazioni dell'editor, in `%APPDATA%\it.overzoom.steroitor`.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("it.overzoom.steroitor"))
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub folder: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub active: usize,
    pub explorer_width: Option<i32>,
    pub explorer_visible: Option<bool>,
    /// left, top, right, bottom in pixel dello schermo.
    pub window: Option<[i32; 4]>,
    pub maximized: bool,
    pub wrap: bool,
}

fn session_file() -> Option<PathBuf> {
    Some(data_dir()?.join("native").join("session.json"))
}

impl Session {
    pub fn load() -> Session {
        session_file().and_then(|file| std::fs::read_to_string(file).ok()).and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(file) = session_file() else { return };
        let Ok(text) = serde_json::to_string_pretty(self) else { return };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let temporary = file.with_extension("json.tmp");
        if std::fs::write(&temporary, text).is_ok() {
            let _ = std::fs::rename(temporary, file);
        }
    }
}

/// Le stesse impostazioni Editor della versione Tauri (`preferences/settings.json`).
#[derive(Deserialize, Clone, Copy)]
#[serde(default, rename_all = "camelCase")]
pub struct EditorStyle {
    pub font_size: f64,
    pub tab_size: usize,
    pub indent_size: usize,
    pub use_tabs: bool,
}

impl Default for EditorStyle {
    fn default() -> Self {
        EditorStyle { font_size: 14.0, tab_size: 4, indent_size: 2, use_tabs: false }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Settings {
    editor: EditorStyle,
}

impl EditorStyle {
    pub fn load() -> EditorStyle {
        data_dir()
            .and_then(|dir| std::fs::read_to_string(dir.join("preferences").join("settings.json")).ok())
            .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
            .map(|settings| settings.editor)
            .unwrap_or_default()
    }

    /// Dimensione in centesimi di punto per SCI_STYLESETSIZEFRACTIONAL: i px della versione web sono a 96 DPI.
    pub fn font_size_fraction(&self) -> isize {
        (self.font_size.clamp(6.0, 72.0) * 75.0).round() as isize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_partial_settings_with_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"editor":{"fontSize":16,"useTabs":true},"other":1}"#).unwrap();
        assert_eq!(settings.editor.font_size, 16.0);
        assert!(settings.editor.use_tabs);
        assert_eq!(settings.editor.tab_size, 4);
        assert_eq!(settings.editor.font_size_fraction(), 1200);
    }

    #[test]
    fn tolerates_an_old_or_partial_session() {
        let session: Session = serde_json::from_str(r#"{"files":["C:\\a.txt"]}"#).unwrap();
        assert_eq!(session.files, vec![PathBuf::from("C:\\a.txt")]);
        assert!(session.folder.is_none());
    }
}

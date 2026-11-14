//! Sessione (cartella, tab aperte, layout) e impostazioni dell'editor, in `%APPDATA%\it.overzoom.steroitor`.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("it.overzoom.steroitor"))
}

fn settings_file() -> Option<PathBuf> {
    Some(data_dir()?.join("preferences").join("settings.json"))
}

/// Scrive in un file temporaneo e lo rinomina: un'interruzione non lascia il file a metà.
fn write_atomic(file: &std::path::Path, text: &str) {
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let temporary = file.with_extension("json.tmp");
    if std::fs::write(&temporary, text).is_ok() {
        let _ = std::fs::rename(temporary, file);
    }
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
        write_atomic(&file, &text);
    }
}

/// Le stesse impostazioni Editor della versione Tauri (`preferences/settings.json`).
#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct EditorStyle {
    pub font_size: f64,
    pub line_height: f64,
    pub tab_size: usize,
    pub indent_size: usize,
    pub use_tabs: bool,
}

impl Default for EditorStyle {
    fn default() -> Self {
        EditorStyle { font_size: 14.0, line_height: 1.5, tab_size: 4, indent_size: 2, use_tabs: false }
    }
}

/// Le impostazioni della versione Tauri che valgono anche qui.
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub editor: EditorStyle,
    /// "merge" (default) o "rebase", per il pull.
    pub update_method: String,
    /// Id della shell scelta (`pwsh`, `cmd`, `git-bash`, …); vuoto per la prima trovata.
    pub terminal_shell: String,
    /// Id dei plugin spenti, come nella versione Tauri.
    pub disabled_plugins: Vec<String>,
    /// Id plugin → id opzione → attiva; un'opzione mai toccata è spenta.
    pub plugin_options: HashMap<String, HashMap<String, bool>>,
}

impl Settings {
    pub fn load() -> Settings {
        settings_file()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Il file è condiviso con la versione Tauri: si cambiano solo le chiavi gestite qui, il resto resta com'è.
    pub fn save(&self) {
        let Some(file) = settings_file() else { return };
        let saved = std::fs::read_to_string(&file).ok().and_then(|text| serde_json::from_str(&text).ok());
        let merged = self.merge_into(saved.unwrap_or_else(|| serde_json::json!({})));
        if let Ok(text) = serde_json::to_string_pretty(&merged) {
            write_atomic(&file, &text);
        }
    }

    fn merge_into(&self, mut saved: serde_json::Value) -> serde_json::Value {
        if !saved.is_object() {
            saved = serde_json::json!({});
        }
        let style = &self.editor;
        let editor = &mut saved["editor"];
        if !editor.is_object() {
            *editor = serde_json::json!({});
        }
        editor["fontSize"] = style.font_size.into();
        editor["lineHeight"] = style.line_height.into();
        editor["tabSize"] = style.tab_size.into();
        editor["indentSize"] = style.indent_size.into();
        editor["useTabs"] = style.use_tabs.into();
        saved["updateMethod"] = (if self.pull_rebase() { "rebase" } else { "merge" }).into();
        saved["terminalShell"] = self.terminal_shell.clone().into();
        saved["disabledPlugins"] = self.disabled_plugins.clone().into();
        // Opzione per opzione: restano quelle che conosce solo la versione Tauri (es. `jdtls`).
        let options = &mut saved["pluginOptions"];
        if !options.is_object() {
            *options = serde_json::json!({});
        }
        for (plugin, values) in &self.plugin_options {
            if !options[plugin].is_object() {
                options[plugin] = serde_json::json!({});
            }
            for (option, enabled) in values {
                options[plugin][option] = (*enabled).into();
            }
        }
        saved
    }

    pub fn plugin_enabled(&self, id: &str) -> bool {
        !self.disabled_plugins.iter().any(|disabled| disabled == id)
    }

    pub fn plugin_option(&self, id: &str, option: &str) -> bool {
        self.plugin_options.get(id).and_then(|options| options.get(option)).copied().unwrap_or(false)
    }

    pub fn pull_rebase(&self) -> bool {
        self.update_method == "rebase"
    }
}

impl EditorStyle {
    /// Dimensione in centesimi di punto per SCI_STYLESETSIZEFRACTIONAL: i px della versione web sono a 96 DPI.
    pub fn font_size_fraction(&self) -> isize {
        (self.font_size.clamp(6.0, 72.0) * 75.0).round() as isize
    }

    /// Spazio in più sopra e sotto ogni riga (SCI_SETEXTRAASCENT/DESCENT) per l'interlinea, in pixel reali.
    pub fn line_spacing(&self, dpi: i32) -> (i32, i32) {
        let extra = ((self.line_height.clamp(1.0, 2.5) - 1.0) * self.font_size * f64::from(dpi) / 96.0).round() as i32;
        (extra / 2, extra - extra / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_partial_settings_with_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"editor":{"fontSize":16,"useTabs":true},"updateMethod":"rebase","other":1}"#).unwrap();
        assert!(settings.pull_rebase());
        assert_eq!(settings.editor.font_size, 16.0);
        assert!(settings.editor.use_tabs);
        assert_eq!(settings.editor.tab_size, 4);
        assert_eq!(settings.editor.font_size_fraction(), 1200);
    }

    #[test]
    fn saving_keeps_the_keys_of_the_tauri_version() {
        let saved = serde_json::json!({"editor": {"fontSize": 12, "other": 1}, "disabledPlugins": ["java"], "pluginOptions": {"java": {"jdtls": true}}, "skippedVersion": "1.2.0"});
        let plugin_options = HashMap::from([("java".to_string(), HashMap::from([("typeCheck".to_string(), true)]))]);
        let settings = Settings { editor: EditorStyle { font_size: 16.0, use_tabs: true, ..EditorStyle::default() }, update_method: "rebase".into(), terminal_shell: "cmd".into(), disabled_plugins: vec!["javascript".into()], plugin_options };
        let merged = settings.merge_into(saved);
        assert_eq!(merged["disabledPlugins"][0], "javascript");
        assert_eq!(merged["pluginOptions"]["java"]["jdtls"], true);
        assert!(settings.plugin_option("java", "typeCheck") && !settings.plugin_option("java", "docs"));
        assert!(settings.plugin_enabled("java") && !settings.plugin_enabled("javascript"));
        assert_eq!(merged["skippedVersion"], "1.2.0");
        assert_eq!(merged["editor"]["other"], 1);
        assert_eq!(merged["editor"]["fontSize"], 16.0);
        assert_eq!(merged["updateMethod"], "rebase");
        let mut reloaded: Settings = serde_json::from_value(merged).unwrap();
        assert_eq!(reloaded.plugin_options.get_mut("java").and_then(|options| options.remove("jdtls")), Some(true));
        assert_eq!(reloaded, settings);
    }

    #[test]
    fn spreads_the_line_height_above_and_below() {
        let style = EditorStyle { font_size: 14.0, line_height: 1.5, ..EditorStyle::default() };
        assert_eq!(style.line_spacing(96), (3, 4));
        assert_eq!(EditorStyle { line_height: 1.0, ..style }.line_spacing(144), (0, 0));
    }

    #[test]
    fn tolerates_an_old_or_partial_session() {
        let session: Session = serde_json::from_str(r#"{"files":["C:\\a.txt"]}"#).unwrap();
        assert_eq!(session.files, vec![PathBuf::from("C:\\a.txt")]);
        assert!(session.folder.is_none());
    }
}

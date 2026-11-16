//! Configurazioni di avvio (pulsante "▶ Esegui"): nome, cartella relativa al progetto e comando, eseguito in un
//! terminale. Stanno in `native\runs.json`, per cartella del progetto, fuori dal repository.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::session::{data_dir, write_atomic};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct RunConfig {
    pub name: String,
    /// Relativa alla cartella del progetto ("." per la cartella stessa).
    pub folder: String,
    pub command: String,
}

type Saved = HashMap<String, Vec<RunConfig>>;

fn file() -> Option<PathBuf> {
    Some(data_dir()?.join("native").join("runs.json"))
}

fn key(project: &Path) -> String {
    project.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase()
}

fn read_all() -> Saved {
    file().and_then(|file| std::fs::read_to_string(file).ok()).and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

pub fn load(project: &Path) -> Vec<RunConfig> {
    read_all().remove(&key(project)).unwrap_or_default()
}

pub fn save(project: &Path, configs: &[RunConfig]) {
    let Some(file) = file() else { return };
    let mut saved = read_all();
    if configs.is_empty() {
        saved.remove(&key(project));
    } else {
        saved.insert(key(project), configs.to_vec());
    }
    if let Ok(text) = serde_json::to_string_pretty(&saved) {
        write_atomic(&file, &text);
    }
}

pub fn folder_of(project: &Path, config: &RunConfig) -> PathBuf {
    project.join(&config.folder)
}

/// Il comando più probabile per la cartella, da correggere nel dialogo.
pub fn suggested_command(dir: &Path) -> &'static str {
    if dir.join("package.json").is_file() {
        "npm run dev"
    } else if dir.join("pom.xml").is_file() {
        "mvn spring-boot:run"
    } else if dir.join("build.gradle").is_file() || dir.join("build.gradle.kts").is_file() {
        ".\\gradlew run"
    } else {
        ""
    }
}

pub fn suggested_name(folder: &str, command: &str) -> String {
    let last = Path::new(folder).file_name().map(|name| name.to_string_lossy().into_owned());
    match last.filter(|name| name != "." && !name.is_empty()) {
        Some(name) => format!("{name}: {command}"),
        None => command.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_names_and_commands() {
        assert_eq!(suggested_name(".\\progetto-fe", "npm run dev"), "progetto-fe: npm run dev");
        assert_eq!(suggested_name(".", "mvn spring-boot:run"), "mvn spring-boot:run");
        let dir = std::env::temp_dir().join(format!("steroitor-run-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(suggested_command(&dir), "");
        std::fs::write(dir.join("pom.xml"), "").unwrap();
        assert_eq!(suggested_command(&dir), "mvn spring-boot:run");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(key(Path::new("C:\\Progetti\\App\\")), key(Path::new("c:\\progetti\\app")));
    }
}

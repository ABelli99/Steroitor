use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
const STATE_FILE: &str = "windows.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProjectRef {
    id: String,
    folder: Option<String>,
}

/// Progetti aperti in una finestra, nell'ordine della barra laterale.
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct WindowState {
    projects: Vec<ProjectRef>,
    active: Option<String>,
}

#[derive(Serialize)]
pub struct Located {
    window: String,
    project: String,
}

struct Entry {
    label: String,
    state: WindowState,
}

/// Finestre e progetti di tutta l'app, salvati in windows.json per riaprirli al prossimo avvio.
pub struct Windows {
    entries: Mutex<Vec<Entry>>,
    next_id: AtomicU32,
    file: Option<PathBuf>,
}

impl Windows {
    fn new(file: Option<PathBuf>, saved: Vec<WindowState>) -> Self {
        let entries = saved.into_iter().enumerate().map(|(index, state)| Entry { label: label_for(index), state }).collect();
        Self { entries: Mutex::new(entries), next_id: AtomicU32::new(1), file }
    }

    pub fn load(app: &AppHandle) -> Self {
        let file = app.path().app_data_dir().ok().map(|dir| dir.join(STATE_FILE));
        let saved = file
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|raw| serde_json::from_str::<Vec<WindowState>>(&raw).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|state| !state.projects.is_empty())
            .collect::<Vec<_>>();
        let windows = Self::new(file, saved);
        windows.next_id.store((windows.labels().len() as u32).max(1), Ordering::Relaxed);
        windows
    }

    fn labels(&self) -> Vec<String> {
        self.entries.lock().map(|entries| entries.iter().map(|entry| entry.label.clone()).collect()).unwrap_or_default()
    }

    fn state_of(&self, label: &str) -> Option<WindowState> {
        let entries = self.entries.lock().ok()?;
        entries.iter().find(|entry| entry.label == label).map(|entry| entry.state.clone())
    }

    fn set(&self, label: &str, state: WindowState) {
        let Ok(mut entries) = self.entries.lock() else { return };
        match entries.iter_mut().find(|entry| entry.label == label) {
            Some(entry) => entry.state = state,
            None => entries.push(Entry { label: label.into(), state }),
        }
    }

    fn locate(&self, folder: &str) -> Option<Located> {
        let wanted = normalize(folder);
        let entries = self.entries.lock().ok()?;
        entries.iter().find_map(|entry| {
            let project = entry.state.projects.iter().find(|project| project.folder.as_deref().map(normalize).as_deref() == Some(wanted.as_str()))?;
            Some(Located { window: entry.label.clone(), project: project.id.clone() })
        })
    }

    /// Chiudere l'ultima finestra chiude l'app: il suo stato resta per il prossimo avvio.
    pub fn window_destroyed(&self, label: &str) {
        let Ok(mut entries) = self.entries.lock() else { return };
        if entries.len() > 1 {
            entries.retain(|entry| entry.label != label);
        }
    }

    pub fn persist(&self) {
        let Some(file) = &self.file else { return };
        let Ok(entries) = self.entries.lock() else { return };
        let states: Vec<&WindowState> = entries.iter().map(|entry| &entry.state).filter(|state| !state.projects.is_empty()).collect();
        let Ok(json) = serde_json::to_string(&states) else { return };
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(file, json);
    }

    fn next_label(&self) -> String {
        format!("project-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }
}

fn label_for(index: usize) -> String {
    if index == 0 { MAIN.into() } else { format!("project-{index}") }
}

fn normalize(path: &str) -> String {
    path.trim_end_matches(['\\', '/']).replace('\\', "/").to_lowercase()
}

fn focus(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn build_window(app: &AppHandle, label: &str, position: Option<(f64, f64)>) -> Result<WebviewWindow, String> {
    let mut config = app.config().app.windows.first().cloned().ok_or("Configurazione finestra mancante")?;
    config.label = label.into();
    config.url = WebviewUrl::App("index.html".into());
    let mut builder = WebviewWindowBuilder::from_config(app, &config).map_err(|e| e.to_string())?;
    if let Some((x, y)) = position {
        builder = builder.position(x, y);
    }
    builder.build().map_err(|e| e.to_string())
}

/// All'avvio la finestra principale è già creata dal config; le altre salvate si ricreano qui.
pub fn restore_extra_windows(app: &AppHandle) {
    for label in app.state::<Windows>().labels().into_iter().filter(|label| label != MAIN) {
        if let Err(error) = build_window(app, &label, None) {
            eprintln!("Finestra {label} non ripristinata: {error}");
        }
    }
}

/// Finestra che riceve i file aperti da un secondo avvio: quella principale, se esiste ancora.
pub fn primary(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN).or_else(|| app.webview_windows().into_values().next())
}

#[tauri::command]
pub fn window_state(window: WebviewWindow, windows: State<Windows>) -> Option<WindowState> {
    windows.state_of(window.label()).filter(|state| !state.projects.is_empty())
}

#[tauri::command]
pub fn set_window_state(window: WebviewWindow, windows: State<Windows>, state: WindowState) {
    windows.set(window.label(), state);
    windows.persist();
}

#[tauri::command]
pub fn locate_project(windows: State<Windows>, folder: String) -> Option<Located> {
    windows.locate(&folder)
}

/// Porta in primo piano la finestra e le chiede di mostrare il progetto.
#[tauri::command]
pub fn activate_project(app: AppHandle, window: String, project: String) -> Result<(), String> {
    let target = app.get_webview_window(&window).ok_or("Finestra chiusa")?;
    focus(&target);
    target.emit_to(window.as_str(), "activate-project", project).map_err(|e| e.to_string())
}

/// Il progetto è già stato chiuso nella finestra di partenza: qui riparte da solo in una finestra nuova.
#[tauri::command]
pub async fn detach_project(app: AppHandle, project: ProjectRef, x: f64, y: f64) -> Result<(), String> {
    let windows = app.state::<Windows>();
    let label = windows.next_label();
    let active = Some(project.id.clone());
    windows.set(&label, WindowState { projects: vec![project], active });
    if let Err(error) = build_window(&app, &label, Some((x, y))) {
        windows.window_destroyed(&label);
        return Err(error);
    }
    windows.persist();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str, folder: &str) -> ProjectRef {
        ProjectRef { id: id.into(), folder: Some(folder.into()) }
    }

    fn state(projects: Vec<ProjectRef>) -> WindowState {
        WindowState { active: projects.first().map(|project| project.id.clone()), projects }
    }

    #[test]
    fn restores_saved_windows_with_main_first() {
        let windows = Windows::new(None, vec![state(vec![project("a", "A")]), state(vec![project("b", "B")])]);
        assert_eq!(windows.labels(), vec!["main", "project-1"]);
        assert_eq!(windows.state_of("project-1"), Some(state(vec![project("b", "B")])));
    }

    #[test]
    fn locates_a_folder_regardless_of_case_and_trailing_separator() {
        let windows = Windows::new(None, vec![]);
        windows.set("project-1", state(vec![project("p1", "C:\\Repos\\App"), project("p2", "C:\\Repos\\Lib")]));
        let found = windows.locate("c:/repos/lib/").expect("progetto trovato");
        assert_eq!((found.window.as_str(), found.project.as_str()), ("project-1", "p2"));
        assert!(windows.locate("C:\\Repos\\Other").is_none());
    }

    #[test]
    fn the_last_window_keeps_its_state_for_the_next_start() {
        let windows = Windows::new(None, vec![state(vec![project("a", "A")]), state(vec![project("b", "B")])]);
        windows.window_destroyed("project-1");
        assert_eq!(windows.labels(), vec!["main"]);
        windows.window_destroyed("main");
        assert_eq!(windows.labels(), vec!["main"]);
    }
}

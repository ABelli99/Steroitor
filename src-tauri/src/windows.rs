use serde::Serialize;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
const CHANGED_EVENT: &str = "projects-changed";

#[derive(Serialize, Clone)]
pub struct ProjectWindow {
    label: String,
    folder: Option<String>,
}

/// Finestre aperte in ordine di creazione, con la cartella su cui lavora ciascuna.
pub struct Projects {
    windows: Mutex<Vec<ProjectWindow>>,
    next_id: AtomicU32,
}

impl Default for Projects {
    fn default() -> Self {
        Self { windows: Mutex::new(vec![ProjectWindow { label: MAIN.into(), folder: None }]), next_id: AtomicU32::new(1) }
    }
}

impl Projects {
    fn list(&self) -> Vec<ProjectWindow> {
        self.windows.lock().map(|windows| windows.clone()).unwrap_or_default()
    }

    pub fn folder_of(&self, label: &str) -> Option<String> {
        self.list().into_iter().find(|window| window.label == label)?.folder
    }

    fn label_with_folder(&self, folder: &str) -> Option<String> {
        let wanted = normalize(folder);
        self.list().into_iter().find(|window| window.folder.as_deref().map(normalize) == Some(wanted.clone())).map(|window| window.label)
    }

    fn upsert(&self, label: &str, folder: Option<String>) {
        let Ok(mut windows) = self.windows.lock() else { return };
        match windows.iter_mut().find(|window| window.label == label) {
            Some(window) => window.folder = folder,
            None => windows.push(ProjectWindow { label: label.into(), folder }),
        }
    }

    pub fn remove(&self, label: &str) {
        if let Ok(mut windows) = self.windows.lock() {
            windows.retain(|window| window.label != label);
        }
    }
}

fn normalize(path: &str) -> String {
    path.trim_end_matches(['\\', '/']).replace('\\', "/").to_lowercase()
}

pub fn broadcast(app: &AppHandle) {
    let _ = app.emit(CHANGED_EVENT, app.state::<Projects>().list());
}

/// Finestra che riceve i file aperti da un secondo avvio: quella principale, se esiste ancora.
pub fn primary(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN).or_else(|| {
        let label = app.state::<Projects>().list().into_iter().next()?.label;
        app.get_webview_window(&label)
    })
}

fn focus(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

#[tauri::command]
pub fn project_windows(projects: State<Projects>) -> Vec<ProjectWindow> {
    projects.list()
}

#[tauri::command]
pub fn window_folder(window: WebviewWindow, projects: State<Projects>) -> Option<String> {
    projects.folder_of(window.label())
}

#[tauri::command]
pub fn set_window_folder(window: WebviewWindow, projects: State<Projects>, folder: Option<String>) {
    projects.upsert(window.label(), folder);
    broadcast(window.app_handle());
}

#[tauri::command]
pub fn focus_project_window(app: AppHandle, label: String) -> Result<(), String> {
    let window = app.get_webview_window(&label).ok_or("Finestra chiusa")?;
    focus(&window);
    Ok(())
}

/// Se la cartella è già aperta in una finestra porta quella in primo piano, altrimenti ne crea una nuova.
#[tauri::command]
pub async fn open_project_window(app: AppHandle, folder: String) -> Result<(), String> {
    let projects = app.state::<Projects>();
    if let Some(window) = projects.label_with_folder(&folder).and_then(|label| app.get_webview_window(&label)) {
        focus(&window);
        return Ok(());
    }

    let label = format!("project-{}", projects.next_id.fetch_add(1, Ordering::Relaxed));
    let mut config = app.config().app.windows.first().cloned().ok_or("Configurazione finestra mancante")?;
    config.label = label.clone();
    config.url = WebviewUrl::App("index.html".into());
    projects.upsert(&label, Some(folder));
    if let Err(error) = WebviewWindowBuilder::from_config(&app, &config).and_then(|builder| builder.build()) {
        projects.remove(&label);
        return Err(error.to_string());
    }
    broadcast(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_folder_regardless_of_case_and_trailing_separator() {
        let projects = Projects::default();
        projects.upsert("project-1", Some("C:\\Repos\\App".into()));
        assert_eq!(projects.label_with_folder("c:/repos/app/").as_deref(), Some("project-1"));
        assert_eq!(projects.label_with_folder("C:\\Repos\\Other"), None);
    }

    #[test]
    fn keeps_creation_order_and_forgets_closed_windows() {
        let projects = Projects::default();
        projects.upsert("project-1", Some("A".into()));
        projects.upsert(MAIN, Some("M".into()));
        projects.remove("project-1");
        let labels: Vec<_> = projects.list().into_iter().map(|window| window.label).collect();
        assert_eq!(labels, vec![MAIN]);
        assert_eq!(projects.folder_of(MAIN).as_deref(), Some("M"));
    }
}

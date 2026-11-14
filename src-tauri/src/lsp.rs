//! Language server esterni (per ora jdtls per Java) su stdio: qui solo processo e framing `Content-Length`,
//! il protocollo JSON-RPC lo gestisce il frontend.
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

const PROJECT_MARKERS: [&str; 5] = ["pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle", ".project"];
const VSCODE_EXTENSION_DIRS: [&str; 3] = [".vscode/extensions", ".vscode-insiders/extensions", ".cursor/extensions"];

struct Server {
    owner: String,
    key: String,
    stdin: ChildStdin,
    child: Child,
}

#[derive(Default)]
pub struct LanguageServers {
    servers: Mutex<HashMap<u32, Server>>,
    next_id: AtomicU32,
}

impl LanguageServers {
    /// Chiude i server di una finestra che non esiste più.
    pub fn close_window(&self, label: &str) {
        let Ok(mut servers) = self.servers.lock() else { return };
        servers.retain(|_, server| {
            if server.owner != label {
                return true;
            }
            let _ = server.child.kill();
            false
        });
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    id: u32,
    root: String,
}

#[derive(Serialize, Clone)]
struct Message {
    id: u32,
    message: String,
}

#[derive(Serialize, Clone)]
struct Exit {
    id: u32,
    error: String,
}

/// La cartella del progetto: la più vicina con pom.xml, build.gradle o simili; altrimenti quella del file.
fn project_root(file: &Path) -> PathBuf {
    let start = file.parent().unwrap_or(file);
    start
        .ancestors()
        .find(|dir| PROJECT_MARKERS.iter().any(|marker| dir.join(marker).exists()))
        .unwrap_or(start)
        .to_path_buf()
}

/// L'estensione Java di VS Code (redhat.java) contiene sia jdtls sia un JRE abbastanza recente.
fn vscode_java_extension() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    VSCODE_EXTENSION_DIRS
        .iter()
        .filter_map(|dir| std::fs::read_dir(Path::new(&home).join(dir)).ok())
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("redhat.java-")))
        .filter(|path| path.join("server").is_dir())
        .max()
}

fn first_match(dir: &Path, accept: impl Fn(&str) -> bool) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()).is_some_and(&accept))
        .max()
}

struct Jdtls {
    java: PathBuf,
    launcher: PathBuf,
    config: PathBuf,
}

const JAVA_EXE: &str = if cfg!(windows) { "java.exe" } else { "java" };
const CONFIG_DIR: &str = if cfg!(windows) { "config_win" } else if cfg!(target_os = "macos") { "config_mac" } else { "config_linux" };

/// jdtls da `JDTLS_HOME` o dall'estensione di VS Code; Java da `JDTLS_JAVA_HOME`, dal JRE dell'estensione, da `JAVA_HOME` o dal PATH.
fn locate_jdtls() -> Result<Jdtls, String> {
    let extension = vscode_java_extension();
    let home = std::env::var_os("JDTLS_HOME").map(PathBuf::from).or_else(|| extension.as_ref().map(|dir| dir.join("server")));
    let home = home.ok_or("jdtls non trovato: imposta JDTLS_HOME o installa l'estensione Java di VS Code (redhat.java)")?;
    let launcher = first_match(&home.join("plugins"), |name| name.starts_with("org.eclipse.equinox.launcher_") && name.ends_with(".jar"))
        .ok_or_else(|| format!("Launcher di jdtls non trovato in {}", home.display()))?;

    let bundled_jre = extension.as_ref().and_then(|dir| first_match(&dir.join("jre"), |_| true)).map(|jre| jre.join("bin").join(JAVA_EXE));
    let java = std::env::var_os("JDTLS_JAVA_HOME")
        .map(|dir| PathBuf::from(dir).join("bin").join(JAVA_EXE))
        .or(bundled_jre.filter(|path| path.exists()))
        .or_else(|| std::env::var_os("JAVA_HOME").map(|dir| PathBuf::from(dir).join("bin").join(JAVA_EXE)))
        .unwrap_or_else(|| PathBuf::from(JAVA_EXE));
    Ok(Jdtls { java, launcher, config: home.join(CONFIG_DIR) })
}

fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

fn jdtls_command(app: &AppHandle, root: &Path) -> Result<Command, String> {
    let jdtls = locate_jdtls()?;
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?.join("jdtls");
    let workspace = base.join("workspaces").join(format!("{:016x}", hash(&root.to_string_lossy().to_lowercase())));
    std::fs::create_dir_all(&workspace).map_err(|e| e.to_string())?;

    let mut command = Command::new(&jdtls.java);
    command
        .args([
            "-Declipse.application=org.eclipse.jdt.ls.core.id1",
            "-Dosgi.bundles.defaultStartLevel=4",
            "-Declipse.product=org.eclipse.jdt.ls.core.product",
            "-Dosgi.checkConfiguration=true",
            "-Dosgi.sharedConfiguration.area.readOnly=true",
            "-Dosgi.configuration.cascaded=true",
            "-Xms64m",
            "-Xmx1G",
            "-XX:+UseSerialGC",
            "--add-modules=ALL-SYSTEM",
            "--add-opens",
            "java.base/java.util=ALL-UNNAMED",
            "--add-opens",
            "java.base/java.lang=ALL-UNNAMED",
        ])
        .arg(format!("-Dosgi.sharedConfiguration.area={}", jdtls.config.display()))
        .arg("-jar")
        .arg(&jdtls.launcher)
        .arg("-configuration")
        .arg(base.join("config"))
        .arg("-data")
        .arg(&workspace)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    Ok(command)
}

/// Legge i messaggi `Content-Length: n\r\n\r\n<n byte>`; None a fine stream.
fn read_message(reader: &mut impl BufRead) -> Option<String> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

/// Avvia (o riusa) il server per il progetto del file; i messaggi arrivano alla finestra come evento `lsp-message`.
#[tauri::command]
pub fn lsp_start(app: AppHandle, window: WebviewWindow, servers: State<LanguageServers>, server: String, file: String) -> Result<Started, String> {
    if server != "jdtls" {
        return Err(format!("Language server sconosciuto: {server}"));
    }
    let root = project_root(Path::new(&file));
    let owner = window.label().to_string();
    let key = format!("{server}:{}", root.to_string_lossy().to_lowercase());
    let root_text = root.to_string_lossy().to_string();

    let mut running = servers.servers.lock().map_err(|e| e.to_string())?;
    if let Some((id, _)) = running.iter().find(|(_, existing)| existing.owner == owner && existing.key == key) {
        return Ok(Started { id: *id, root: root_text });
    }

    let mut child = jdtls_command(&app, &root)?.spawn().map_err(|e| format!("Avvio di jdtls fallito: {e}"))?;
    let id = servers.next_id.fetch_add(1, Ordering::Relaxed);
    let stdin = child.stdin.take().ok_or("stdin di jdtls non disponibile")?;
    let stdout = child.stdout.take().ok_or("stdout di jdtls non disponibile")?;
    let stderr = child.stderr.take().ok_or("stderr di jdtls non disponibile")?;

    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut text);
        text
    });
    let events = window.clone();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        while let Some(message) = read_message(&mut reader) {
            let _ = events.emit_to(events.label(), "lsp-message", Message { id, message });
        }
        let error = errors.join().unwrap_or_default();
        let tail = error.chars().skip(error.chars().count().saturating_sub(2000)).collect();
        let _ = events.emit_to(events.label(), "lsp-exit", Exit { id, error: tail });
        let app = events.app_handle();
        if let Ok(mut servers) = app.state::<LanguageServers>().servers.lock() {
            servers.remove(&id);
        }
    });

    running.insert(id, Server { owner, key, stdin, child });
    Ok(Started { id, root: root_text })
}

#[tauri::command]
pub fn lsp_send(servers: State<LanguageServers>, id: u32, message: String) -> Result<(), String> {
    let mut running = servers.servers.lock().map_err(|e| e.to_string())?;
    let server = running.get_mut(&id).ok_or("Language server non attivo")?;
    write!(server.stdin, "Content-Length: {}\r\n\r\n{}", message.len(), message).map_err(|e| e.to_string())?;
    server.stdin.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_framed_messages() {
        let mut input = Cursor::new(b"Content-Length: 2\r\nContent-Type: x\r\n\r\n{}Content-Length: 4\r\n\r\nnull".to_vec());
        assert_eq!(read_message(&mut input).as_deref(), Some("{}"));
        assert_eq!(read_message(&mut input).as_deref(), Some("null"));
        assert_eq!(read_message(&mut input), None);
    }

    #[test]
    fn finds_the_nearest_project_root() {
        let dir = std::env::temp_dir().join(format!("steroitor-lsp-{}", std::process::id()));
        let source = dir.join("app").join("src").join("main");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(dir.join("app").join("pom.xml"), "").unwrap();
        assert_eq!(project_root(&source.join("Main.java")), dir.join("app"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

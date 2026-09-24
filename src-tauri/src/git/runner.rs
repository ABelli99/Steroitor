use serde::Serialize;
use std::path::{Path, MAIN_SEPARATOR};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const CONSOLE_OUTPUT_LIMIT: usize = 16 * 1024;
const SETTINGS_FILE: &str = "settings.json";

/// Percorso dell'eseguibile git: `gitPath` in settings.json, altrimenti `git` dal PATH.
pub struct GitConfig {
    executable: RwLock<String>,
    next_id: AtomicU64,
}

impl GitConfig {
    pub fn load(app: &AppHandle) -> Self {
        let configured = app
            .path()
            .app_data_dir()
            .ok()
            .and_then(|dir| std::fs::read_to_string(dir.join(SETTINGS_FILE)).ok())
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|settings| settings.get("gitPath")?.as_str().map(str::to_owned));
        Self { executable: RwLock::new(configured.unwrap_or_else(|| "git".into())), next_id: AtomicU64::new(1) }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommandLog {
    id: u64,
    command: String,
    cwd: String,
    code: Option<i32>,
    duration_ms: u128,
    started_at: u128,
    stdout: String,
    stderr: String,
    background: bool,
}

pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub success: bool,
}

impl Output {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

type Sink = Box<dyn Fn(CommandLog) + Send>;

/// Esegue l'eseguibile git reale e notifica ogni comando al `sink` (la Console).
pub struct Git {
    executable: String,
    sink: Sink,
}

impl Git {
    pub fn new(executable: impl Into<String>, sink: Sink) -> Self {
        Self { executable: executable.into(), sink }
    }

    pub fn from_app(app: &AppHandle) -> Self {
        let config = app.state::<GitConfig>();
        let executable = config.executable.read().map(|value| value.clone()).unwrap_or_else(|_| "git".into());
        let app = app.clone();
        Self::new(
            executable,
            Box::new(move |mut log| {
                log.id = app.state::<GitConfig>().next_id.fetch_add(1, Ordering::Relaxed);
                let _ = app.emit("git-command", log);
            }),
        )
    }

    pub fn run(&self, cwd: &Path, args: &[&str], background: bool) -> Result<Output, String> {
        self.run_with_input(cwd, args, None, background)
    }

    /// Come `run`, passando `input` sullo stdin di git.
    pub fn run_with_input(&self, cwd: &Path, args: &[&str], input: Option<&[u8]>, background: bool) -> Result<Output, String> {
        let mut command = Command::new(&self.executable);
        command.args(args).current_dir(cwd).env("GIT_OPTIONAL_LOCKS", "0").env("GIT_TERMINAL_PROMPT", "0").env("GIT_EDITOR", "true");
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let started_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let started = Instant::now();
        let result = match input {
            None => command.output(),
            Some(input) => command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().and_then(|mut child| {
                let mut stdin = child.stdin.take().expect("stdin configurato come piped");
                let data = input.to_vec();
                let writer = std::thread::spawn(move || stdin.write_all(&data));
                let output = child.wait_with_output();
                let _ = writer.join();
                output
            }),
        };
        let duration_ms = started.elapsed().as_millis();

        let (output, stdout, stderr, code) = match result {
            Ok(raw) => {
                let stdout = String::from_utf8_lossy(&raw.stdout).replace('\0', "\u{2400}");
                let stderr = String::from_utf8_lossy(&raw.stderr).into_owned();
                let code = raw.status.code();
                let output = Output { stdout: raw.stdout, stderr: stderr.clone(), success: raw.status.success() };
                (Ok(output), stdout, stderr, code)
            }
            Err(error) => {
                let message = format!("Impossibile eseguire \"{}\": {error}", self.executable);
                (Err(message.clone()), String::new(), message, None)
            }
        };

        (self.sink)(CommandLog {
            id: 0,
            command: std::iter::once("git").chain(args.iter().copied()).map(quote).collect::<Vec<_>>().join(" "),
            cwd: cwd.to_string_lossy().into_owned(),
            code,
            duration_ms,
            started_at,
            stdout: truncate(&stdout),
            stderr: truncate(&stderr),
            background,
        });
        output
    }

    /// Come `run`, ma un exit code diverso da zero diventa un errore con lo stderr.
    pub fn text(&self, cwd: &Path, args: &[&str], background: bool) -> Result<String, String> {
        let output = self.run(cwd, args, background)?;
        if !output.success {
            let message = output.stderr.trim();
            return Err(if message.is_empty() { format!("git {} è fallito", args.join(" ")) } else { message.to_owned() });
        }
        Ok(output.text())
    }
}

fn truncate(text: &str) -> String {
    if text.len() <= CONSOLE_OUTPUT_LIMIT {
        return text.to_owned();
    }
    let mut end = CONSOLE_OUTPUT_LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… (output troncato, {} byte totali)", &text[..end], text.len())
}

fn quote(arg: &str) -> String {
    let visible = arg.replace('\r', "\\r").replace('\n', "\\n");
    if visible.is_empty() || visible.contains([' ', '"', '\t']) {
        format!("\"{}\"", visible.replace('"', "\\\""))
    } else {
        visible
    }
}

pub fn to_native(path: &str) -> String {
    if MAIN_SEPARATOR == '\\' {
        path.replace('/', "\\")
    } else {
        path.to_owned()
    }
}

pub fn native_path(root: &Path, relative: &str) -> String {
    to_native(&root.join(relative).to_string_lossy())
}

/// Percorso relativo alla radice del repo, con `/` come vuole git.
pub fn repo_relative(root: &Path, path: &str) -> Result<String, String> {
    let relative = Path::new(path).strip_prefix(root).map_err(|_| format!("{path} non è nel repository"))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

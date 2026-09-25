use portable_pty::{native_pty_system, Child, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use crate::shells;
use tauri::{Emitter, Manager, State, WebviewWindow};

const READ_BUFFER: usize = 8 * 1024;

struct Session {
    owner: String,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

#[derive(Default)]
pub struct Terminals {
    sessions: Mutex<HashMap<u32, Session>>,
    next_id: AtomicU32,
}

impl Terminals {
    /// Chiude le shell di una finestra che non esiste più.
    pub fn close_window(&self, label: &str) {
        let Ok(mut sessions) = self.sessions.lock() else { return };
        sessions.retain(|_, session| {
            if session.owner != label {
                return true;
            }
            let _ = session.killer.kill();
            false
        });
    }
}

#[derive(Serialize, Clone)]
struct TerminalOutput {
    project: String,
    id: u32,
    data: String,
}

#[derive(Serialize, Clone)]
struct TerminalExit {
    project: String,
    id: u32,
    code: Option<u32>,
}

/// Una lettura dalla PTY può spezzare un carattere UTF-8: i byte incompleti aspettano la lettura successiva.
#[derive(Default)]
struct Utf8Chunker {
    pending: Vec<u8>,
}

impl Utf8Chunker {
    fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let complete = match std::str::from_utf8(&self.pending) {
            Ok(_) => self.pending.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => self.pending.len(),
        };
        let text = String::from_utf8_lossy(&self.pending[..complete]).into_owned();
        self.pending.drain(..complete);
        text
    }
}

struct Spawned {
    session: Session,
    reader: Box<dyn Read + Send>,
    child: Box<dyn Child + Send + Sync>,
}

fn spawn(owner: &str, mut command: CommandBuilder, cwd: Option<&str>, cols: u16, rows: u16) -> Result<Spawned, String> {
    let pair = native_pty_system()
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())?;
    if let Some(cwd) = cwd {
        command.cwd(cwd);
    }
    command.env("TERM", "xterm-256color");
    let child = pair.slave.spawn_command(command).map_err(|e| e.to_string())?;
    drop(pair.slave);

    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let killer = child.clone_killer();
    Ok(Spawned { session: Session { owner: owner.to_owned(), master: pair.master, writer, killer }, reader, child })
}

#[derive(Serialize)]
pub struct OpenedTerminal {
    id: u32,
    shell: String,
}

#[tauri::command]
pub fn terminal_open(
    window: WebviewWindow,
    terminals: State<Terminals>,
    project: String,
    cwd: Option<String>,
    shell: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<OpenedTerminal, String> {
    let shell = shells::resolve(shell.as_deref())?;
    let Spawned { session, mut reader, mut child } = spawn(window.label(), shell.command(), cwd.as_deref(), cols, rows)?;
    let app = window.app_handle().clone();
    let label = window.label().to_owned();
    let id = terminals.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    terminals.sessions.lock().map_err(|e| e.to_string())?.insert(id, session);

    let output = app.clone();
    let output_label = label.clone();
    let output_project = project.clone();
    std::thread::spawn(move || {
        let mut chunker = Utf8Chunker::default();
        let mut buffer = [0u8; READ_BUFFER];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            let data = chunker.push(&buffer[..count]);
            if !data.is_empty() {
                let _ = output.emit_to(output_label.as_str(), "terminal-output", TerminalOutput { project: output_project.clone(), id, data });
            }
        }
    });

    std::thread::spawn(move || {
        let code = child.wait().ok().map(|status| status.exit_code());
        if let Ok(mut sessions) = app.state::<Terminals>().sessions.lock() {
            sessions.remove(&id);
        }
        let _ = app.emit_to(label.as_str(), "terminal-exit", TerminalExit { project, id, code });
    });
    Ok(OpenedTerminal { id, shell: shell.name })
}

fn with_session<T>(terminals: &State<Terminals>, id: u32, action: impl FnOnce(&mut Session) -> Result<T, String>) -> Result<T, String> {
    let mut sessions = terminals.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions.get_mut(&id).ok_or("Terminale chiuso")?;
    action(session)
}

#[tauri::command]
pub fn terminal_write(terminals: State<Terminals>, id: u32, data: String) -> Result<(), String> {
    with_session(&terminals, id, |session| {
        session.writer.write_all(data.as_bytes()).and_then(|_| session.writer.flush()).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn terminal_resize(terminals: State<Terminals>, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    with_session(&terminals, id, |session| {
        session.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn terminal_close(terminals: State<Terminals>, id: u32) -> Result<(), String> {
    let session = terminals.sessions.lock().map_err(|e| e.to_string())?.remove(&id);
    if let Some(mut session) = session {
        let _ = session.killer.kill();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_split_utf8_characters_for_the_next_read() {
        let mut chunker = Utf8Chunker::default();
        let bytes = "città".as_bytes();
        let split = bytes.len() - 1;
        assert_eq!(chunker.push(&bytes[..split]), "citt");
        assert_eq!(chunker.push(&bytes[split..]), "à");
    }

    #[test]
    fn replaces_invalid_bytes_instead_of_waiting_forever() {
        let mut chunker = Utf8Chunker::default();
        assert_eq!(chunker.push(&[b'a', 0xFF, b'b']), "a\u{FFFD}b");
        assert_eq!(chunker.push(b"c"), "c");
    }

    #[test]
    fn runs_a_command_in_a_real_pty() {
        #[cfg(target_os = "windows")]
        let command = {
            let mut command = CommandBuilder::new("cmd.exe");
            command.args(["/C", "echo steroitor-pty"]);
            command
        };
        #[cfg(not(target_os = "windows"))]
        let command = {
            let mut command = CommandBuilder::new("sh");
            command.args(["-c", "echo steroitor-pty"]);
            command
        };

        let Spawned { session, mut reader, mut child } = spawn("test", command, None, 80, 24).unwrap();
        let Session { master, mut writer, mut killer, .. } = session;
        let (found, received) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let mut output = String::new();
            let mut answered = false;
            let mut buffer = [0u8; 1024];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                output.push_str(&String::from_utf8_lossy(&buffer[..count]));
                // ConPTY chiede la posizione del cursore e aspetta la risposta (xterm.js la dà da solo).
                if !answered && output.contains("\x1b[6n") {
                    answered = true;
                    let _ = writer.write_all(b"\x1b[1;1R").and_then(|_| writer.flush());
                }
                if output.contains("steroitor-pty") {
                    let _ = found.send(output.clone());
                    return;
                }
            }
            let _ = found.send(output);
        });

        let output = received.recv_timeout(std::time::Duration::from_secs(20));
        let _ = killer.kill();
        let _ = child.wait();
        drop(master);
        assert!(output.expect("nessun output dalla PTY entro 20 s").contains("steroitor-pty"));
    }
}

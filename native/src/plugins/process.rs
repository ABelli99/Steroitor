//! Processo del server: un thread scrive i messaggi su stdin, uno legge quelli `Content-Length` da stdout e li mette
//! in coda per la finestra, che li interpreta sul thread della UI.
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_PLUGIN_EVENT: u32 = WM_APP + 30;

pub enum Incoming {
    Message(u32, serde_json::Value),
    /// Il server si è chiuso: id e coda dello stderr.
    Exited(u32, String),
}

pub type Queue = Arc<Mutex<Vec<Incoming>>>;

pub struct Process {
    child: Child,
    input: Sender<Vec<u8>>,
}

impl Process {
    pub fn spawn(mut command: Command, id: u32, queue: Queue, hwnd: HWND) -> Result<Process, String> {
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|error| error.to_string())?;
        let (Some(mut stdin), Some(stdout), Some(stderr)) = (child.stdin.take(), child.stdout.take(), child.stderr.take()) else {
            let _ = child.kill();
            return Err("pipe del processo non disponibili".into());
        };
        let (input, outgoing) = channel::<Vec<u8>>();
        std::thread::spawn(move || {
            for bytes in outgoing {
                if stdin.write_all(&bytes).and_then(|_| stdin.flush()).is_err() {
                    break;
                }
            }
        });
        let errors = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut text);
            text
        });
        let target = hwnd as usize;
        std::thread::spawn(move || {
            let post = |incoming: Incoming| {
                if let Ok(mut queue) = queue.lock() {
                    queue.push(incoming);
                }
                // SAFETY: PostMessageW è sicura da qualsiasi thread; se la finestra non esiste più non fa nulla.
                unsafe { PostMessageW(target as HWND, WM_PLUGIN_EVENT, 0, 0) };
            };
            let mut reader = BufReader::new(stdout);
            while let Some(body) = read_message(&mut reader) {
                if let Ok(value) = serde_json::from_str(body.trim_end()) {
                    post(Incoming::Message(id, value));
                }
            }
            let error = errors.join().unwrap_or_default();
            let tail = error.chars().skip(error.chars().count().saturating_sub(2000)).collect();
            post(Incoming::Exited(id, tail));
        });
        Ok(Process { child, input })
    }

    pub fn write(&self, bytes: Vec<u8>) {
        let _ = self.input.send(bytes);
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

/// Legge un messaggio `Content-Length: n\r\n\r\n<n byte>`; None a fine stream.
fn read_message(reader: &mut impl BufRead) -> Option<String> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            if length.is_some() {
                break;
            }
            continue;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    Some(String::from_utf8_lossy(&body).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_framed_messages() {
        let mut input = Cursor::new(b"Content-Length: 2\r\nContent-Type: x\r\n\r\n{}\r\nContent-Length: 5\r\n\r\nnull\n".to_vec());
        assert_eq!(read_message(&mut input).as_deref(), Some("{}"));
        assert_eq!(read_message(&mut input).as_deref(), Some("null\n"));
        assert_eq!(read_message(&mut input), None);
    }
}

//! Terminale integrato: la shell gira in una ConPTY, `vt` emula lo schermo, un controllo Scintilla in sola
//! lettura lo mostra (selezione, copia e scroll sono i suoi).
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use steroitor_core::shells::Shell;
use vte::Parser;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, RECT};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows_sys::Win32::System::Console::{ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::{clipboard_text, wide};
use crate::sci::*;
use crate::theme::Palette;
use crate::vt::{render, Color, Pen, Screen, SCROLLBACK};

pub const WM_TERMINAL_OUTPUT: u32 = WM_APP + 20;
pub const WM_TERMINAL_EXIT: u32 = WM_APP + 21;

const READ_BUFFER: usize = 8 * 1024;
const SCI_SETREADONLY: u32 = 2171;
const SCI_SETUNDOCOLLECTION: u32 = 2012;
const SCI_GETLINECOUNT: u32 = 2154;
const SCI_POSITIONFROMLINE: u32 = 2167;
const SCI_DELETERANGE: u32 = 2645;
const SCI_STARTSTYLING: u32 = 2032;
const SCI_SETSTYLINGEX: u32 = 2073;
const SCI_SETEMPTYSELECTION: u32 = 2556;
const SCI_GETSELECTIONEMPTY: u32 = 2650;
const SCI_LINESONSCREEN: u32 = 2370;
const SCI_SETHSCROLLBAR: u32 = 2130;
const SCI_TEXTHEIGHT: u32 = 2279;
const SCI_STYLESETUNDERLINE: u32 = 2059;
const SCI_SETCARETSTYLE: u32 = 2512;
const SCI_COPY: u32 = 2178;
const CARETSTYLE_INVISIBLE: usize = 0;
const CARETSTYLE_BLOCK: usize = 2;
const STYLE_LAST_PREDEFINED: u8 = 39;

/// I 16 colori ANSI del terminale di VS Code (0xRRGGBB), scuro e chiaro.
const ANSI_DARK: [u32; 16] = [
    0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5, 0x666666, 0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xe5e5e5,
];
const ANSI_LIGHT: [u32; 16] = [
    0x000000, 0xcd3131, 0x00bc00, 0x949800, 0x0451a5, 0xbc05bc, 0x0598bc, 0x555555, 0x666666, 0xcd3131, 0x14ce14, 0xb5ba00, 0x0451a5, 0xbc05bc, 0x0598bc, 0xa5a5a5,
];

fn bgr(r: u8, g: u8, b: u8) -> u32 {
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16)
}

/// Colore Win32 (0x00BBGGRR) di un indice della tavolozza xterm a 256 colori.
fn indexed(index: u8, dark: bool) -> u32 {
    let hex = |value: u32| bgr((value >> 16) as u8, (value >> 8) as u8, value as u8);
    match index {
        0..=15 => hex(if dark { ANSI_DARK } else { ANSI_LIGHT }[index as usize]),
        16..=231 => {
            let level = |value: u8| if value == 0 { 0 } else { 55 + value * 40 };
            let cube = index - 16;
            bgr(level(cube / 36), level(cube / 6 % 6), level(cube % 6))
        }
        _ => {
            let grey = 8 + (index - 232) * 10;
            bgr(grey, grey, grey)
        }
    }
}

/// HANDLE non è Send: i thread ricevono il valore e ne diventano gli unici proprietari.
struct Owned(usize);

/// Shell in una pseudo console. Chiudere la pseudo console termina anche la shell.
struct Pty {
    console: HPCON,
    input: HANDLE,
}

impl Pty {
    fn spawn(shell: &Shell, cwd: Option<&Path>, size: COORD, hwnd: HWND, id: u32, pending: Arc<Mutex<Vec<u8>>>) -> Result<Pty, String> {
        let mut command_line = std::iter::once(&shell.program).chain(&shell.args).map(|part| if part.contains(' ') { format!("\"{part}\"") } else { part.clone() }).collect::<Vec<_>>().join(" ");
        command_line.push('\0');
        let mut command: Vec<u16> = command_line.encode_utf16().collect();
        let cwd = cwd.map(|cwd| wide(&cwd.to_string_lossy()));
        // SAFETY: handle e strutture create qui; ogni handle ha un solo proprietario che lo chiude.
        unsafe {
            let (mut input_read, mut input_write, mut output_read, mut output_write) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            if CreatePipe(&mut input_read, &mut input_write, std::ptr::null(), 0) == 0 {
                return Err("Impossibile creare la pipe del terminale".into());
            }
            if CreatePipe(&mut output_read, &mut output_write, std::ptr::null(), 0) == 0 {
                CloseHandle(input_read);
                CloseHandle(input_write);
                return Err("Impossibile creare la pipe del terminale".into());
            }
            let mut console: HPCON = 0;
            let created = CreatePseudoConsole(size, input_read, output_write, 0, &mut console);
            CloseHandle(input_read);
            CloseHandle(output_write);
            if created < 0 {
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(format!("ConPTY non disponibile (0x{created:08X})"));
            }
            let pty = Pty { console, input: input_write };

            let mut size = 0usize;
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
            let mut attributes = vec![0u8; size];
            let list = attributes.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
            InitializeProcThreadAttributeList(list, 1, 0, &mut size);
            UpdateProcThreadAttribute(list, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize, console as *const _, std::mem::size_of::<HPCON>(), std::ptr::null_mut(), std::ptr::null());
            let mut startup: STARTUPINFOEXW = std::mem::zeroed();
            startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
            // Senza handle standard la shell usa per forza la pseudo console.
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup.lpAttributeList = list;
            let mut process: PROCESS_INFORMATION = std::mem::zeroed();
            let started = CreateProcessW(
                std::ptr::null(),
                command.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                EXTENDED_STARTUPINFO_PRESENT,
                std::ptr::null(),
                cwd.as_ref().map_or(std::ptr::null(), |cwd| cwd.as_ptr()),
                &startup.StartupInfo,
                &mut process,
            );
            DeleteProcThreadAttributeList(list);
            if started == 0 {
                CloseHandle(output_read);
                return Err(format!("Impossibile avviare {}: {}", shell.program, std::io::Error::last_os_error()));
            }
            CloseHandle(process.hThread);

            let (output, target) = (Owned(output_read as usize), hwnd as usize);
            std::thread::spawn(move || {
                let output = output;
                let mut buffer = vec![0u8; READ_BUFFER];
                let mut read = 0u32;
                while ReadFile(output.0 as HANDLE, buffer.as_mut_ptr(), READ_BUFFER as u32, &mut read, std::ptr::null_mut()) != 0 && read > 0 {
                    let Ok(mut pending) = pending.lock() else { break };
                    let first = pending.is_empty();
                    pending.extend_from_slice(&buffer[..read as usize]);
                    drop(pending);
                    if first {
                        PostMessageW(target as HWND, WM_TERMINAL_OUTPUT, id as usize, 0);
                    }
                }
                CloseHandle(output.0 as HANDLE);
            });
            let child = Owned(process.hProcess as usize);
            std::thread::spawn(move || {
                let child = child;
                let mut code = 0u32;
                WaitForSingleObject(child.0 as HANDLE, INFINITE);
                GetExitCodeProcess(child.0 as HANDLE, &mut code);
                CloseHandle(child.0 as HANDLE);
                PostMessageW(target as HWND, WM_TERMINAL_EXIT, id as usize, code as isize);
            });
            Ok(pty)
        }
    }

    fn write(&self, bytes: &[u8]) {
        let mut written = 0u32;
        // SAFETY: handle della pipe di input, valido finché la Pty vive.
        unsafe { WriteFile(self.input, bytes.as_ptr(), bytes.len() as u32, &mut written, std::ptr::null_mut()) };
    }

    fn resize(&self, size: COORD) {
        // SAFETY: pseudo console valida finché la Pty vive.
        unsafe { ResizePseudoConsole(self.console, size) };
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // SAFETY: handle posseduti dalla Pty; il thread di lettura riceve la fine dei dati e chiude il suo.
        unsafe {
            ClosePseudoConsole(self.console);
            CloseHandle(self.input);
        }
    }
}

pub struct Terminal {
    pub id: u32,
    pub title: String,
    pub sci: Sci,
    pub exited: Cell<bool>,
    dark: bool,
    fore: u32,
    back: u32,
    screen: RefCell<Screen>,
    parser: RefCell<Parser>,
    pty: RefCell<Option<Pty>>,
    pending: Arc<Mutex<Vec<u8>>>,
    styles: RefCell<HashMap<Pen, u8>>,
    /// Righe di scrollback nel documento, prima di quelle dello schermo.
    history: Cell<usize>,
    high_surrogate: Cell<u16>,
}

impl Terminal {
    pub fn open(parent: HWND, instance: HINSTANCE, palette: &Palette, id: u32, shell: &Shell, cwd: Option<&Path>) -> Result<Terminal, String> {
        let class = wide("Scintilla");
        // SAFETY: controllo figlio della finestra principale.
        let hwnd = unsafe { CreateWindowExW(0, class.as_ptr(), std::ptr::null(), WS_CHILD | WS_VSCROLL, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null()) };
        if palette.dark {
            let theme = wide("DarkMode_Explorer");
            // SAFETY: controllo valido appena creato.
            unsafe { SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null()) };
        }
        let sci = Sci::new(hwnd);
        sci.call_str(SCI_STYLESETFONT, STYLE_DEFAULT, "Cascadia Mono");
        sci.call(SCI_STYLESETSIZEFRACTIONAL, STYLE_DEFAULT, 975);
        sci.call(SCI_STYLESETFORE, STYLE_DEFAULT, palette.fg as isize);
        sci.call(SCI_STYLESETBACK, STYLE_DEFAULT, palette.editor as isize);
        sci.call(SCI_STYLECLEARALL, 0, 0);
        sci.call(SCI_SETMARGINWIDTHN, 0, 0);
        sci.call(SCI_SETMARGINWIDTHN, 1, 0);
        sci.call(SCI_SETMARGINLEFT, 0, 4);
        sci.call(SCI_SETCODEPAGE, SC_CP_UTF8, 0);
        sci.call(SCI_SETUNDOCOLLECTION, 0, 0);
        sci.call(SCI_SETHSCROLLBAR, 0, 0);
        sci.call(SCI_SETCARETSTYLE, CARETSTYLE_BLOCK, 0);
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_CARET, (palette.fg | 0xFF00_0000) as isize);
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_BACK, (palette.selection | 0xFF00_0000) as isize);
        sci.call(SCI_SETREADONLY, 1, 0);

        let pending = Arc::new(Mutex::new(Vec::new()));
        let screen = Screen::new(80, 24);
        let pty = Pty::spawn(shell, cwd, COORD { X: 80, Y: 24 }, parent, id, Arc::clone(&pending));
        let pty = match pty {
            Ok(pty) => pty,
            Err(error) => {
                // SAFETY: il controllo non serve più.
                unsafe { DestroyWindow(hwnd) };
                return Err(error);
            }
        };
        Ok(Terminal {
            id,
            title: shell.name.clone(),
            sci,
            exited: Cell::new(false),
            dark: palette.dark,
            fore: palette.fg,
            back: palette.editor,
            screen: RefCell::new(screen),
            parser: RefCell::new(Parser::new()),
            pty: RefCell::new(Some(pty)),
            pending,
            styles: RefCell::new(HashMap::new()),
            history: Cell::new(0),
            high_surrogate: Cell::new(0),
        })
    }

    pub fn close(&self) {
        self.pty.borrow_mut().take();
        // SAFETY: controllo creato da questo terminale.
        unsafe { DestroyWindow(self.sci.hwnd) };
    }

    fn write(&self, bytes: &[u8]) {
        if let Some(pty) = self.pty.borrow().as_ref() {
            pty.write(bytes);
        }
    }

    /// Colonne e righe che stanno nel controllo, adattate alla sua dimensione attuale.
    pub fn fit(&self) {
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: controllo valido.
        unsafe { GetClientRect(self.sci.hwnd, &mut client) };
        let width = self.sci.call_str(SCI_TEXTWIDTH, STYLE_DEFAULT, "M").max(1) as i32;
        let height = self.sci.call(SCI_TEXTHEIGHT, 0, 0).max(1) as i32;
        let (cols, rows) = ((client.right - 4) / width, client.bottom / height);
        if cols < 2 || rows < 1 {
            return;
        }
        let mut screen = self.screen.borrow_mut();
        if (screen.cols, screen.rows) == (cols as usize, rows as usize) {
            return;
        }
        screen.resize(cols as usize, rows as usize);
        drop(screen);
        if let Some(pty) = self.pty.borrow().as_ref() {
            pty.resize(COORD { X: cols as i16, Y: rows as i16 });
        }
        self.sync();
    }

    pub fn on_output(&self) {
        let bytes = self.pending.lock().map(|mut pending| std::mem::take(&mut *pending)).unwrap_or_default();
        self.feed(&bytes);
    }

    pub fn on_exit(&self, code: u32) {
        self.exited.set(true);
        self.pty.borrow_mut().take();
        self.feed(format!("\r\n\x1b[90m[processo terminato, codice {code}]\x1b[0m\r\n").as_bytes());
    }

    fn feed(&self, bytes: &[u8]) {
        let replies = {
            let mut screen = self.screen.borrow_mut();
            self.parser.borrow_mut().advance(&mut *screen, bytes);
            std::mem::take(&mut screen.replies)
        };
        if !replies.is_empty() {
            self.write(&replies);
        }
        self.sync();
    }

    fn color(&self, color: Color, default: isize) -> isize {
        match color {
            Color::Default => default,
            Color::Indexed(index) => indexed(index, self.dark) as isize,
            Color::Rgb(r, g, b) => bgr(r, g, b) as isize,
        }
    }

    /// Stile Scintilla di una penna, creato al primo uso.
    fn style(&self, pen: Pen) -> u8 {
        if pen == Pen::default() {
            return 0;
        }
        let mut styles = self.styles.borrow_mut();
        if let Some(&style) = styles.get(&pen) {
            return style;
        }
        // Liberi: 1..=31 e 40..=255 (0 è la penna predefinita, 32..=39 sono gli stili riservati di Scintilla).
        let count = styles.len();
        // shortcut: oltre 247 combinazioni le nuove restano senza colori; basta per shell e TUI comuni.
        if count >= 31 + 255 - STYLE_LAST_PREDEFINED as usize {
            return 0;
        }
        let style = if count < 31 { count + 1 } else { count - 31 + STYLE_LAST_PREDEFINED as usize + 1 } as u8;
        let (mut fg, mut bg) = (self.color(pen.fg, self.fore as isize), self.color(pen.bg, self.back as isize));
        if pen.inverse {
            std::mem::swap(&mut fg, &mut bg);
        }
        let index = style as usize;
        self.sci.call(SCI_STYLESETFORE, index, fg);
        self.sci.call(SCI_STYLESETBACK, index, bg);
        self.sci.call(SCI_STYLESETBOLD, index, pen.bold as isize);
        self.sci.call(SCI_STYLESETUNDERLINE, index, pen.underline as isize);
        styles.insert(pen, style);
        style
    }

    /// Il documento è lo scrollback seguito dallo schermo: si riscrive lo schermo, preceduto dalle righe appena
    /// uscite in cima, e si tagliano le righe più vecchie oltre SCROLLBACK.
    fn sync(&self) {
        let sci = &self.sci;
        let mut screen = self.screen.borrow_mut();
        let line_count = sci.call(SCI_GETLINECOUNT, 0, 0);
        let on_screen = sci.call(SCI_LINESONSCREEN, 0, 0);
        let at_bottom = sci.call(SCI_GETFIRSTVISIBLELINE, 0, 0) + on_screen >= line_count;
        let mut history = self.history.get();
        sci.call(SCI_SETREADONLY, 0, 0);
        if std::mem::take(&mut screen.clear_scrollback) {
            sci.call(SCI_DELETERANGE, 0, sci.call(SCI_POSITIONFROMLINE, history, 0));
            history = 0;
        }
        let start = sci.call(SCI_POSITIONFROMLINE, history, 0);
        let start = if start < 0 { sci.call(SCI_GETLENGTH, 0, 0) } else { start } as usize;

        let (mut text, mut pens) = (Vec::new(), Vec::new());
        history += screen.scrollback.len();
        for line in screen.scrollback.drain(..) {
            render(&line, 0, &mut text, &mut pens);
            text.push(b'\n');
            pens.push(Pen::default());
        }
        let mut cursor = 0;
        for (row, line) in screen.lines.iter().enumerate() {
            if row > 0 {
                text.push(b'\n');
                pens.push(Pen::default());
            }
            let keep = if row == screen.row { screen.col } else { 0 };
            if row == screen.row {
                cursor = start + text.len() + line[..keep].iter().map(|cell| cell.ch.len_utf8()).sum::<usize>();
            }
            render(line, keep, &mut text, &mut pens);
        }
        sci.call(SCI_SETTARGETRANGE, start, sci.call(SCI_GETLENGTH, 0, 0));
        sci.call(SCI_REPLACETARGET, text.len(), text.as_ptr() as isize);
        let styles: Vec<u8> = pens.iter().map(|&pen| self.style(pen)).collect();
        sci.call(SCI_STARTSTYLING, start, 0);
        sci.call(SCI_SETSTYLINGEX, styles.len(), styles.as_ptr() as isize);
        if history > SCROLLBACK {
            let removed = sci.call(SCI_POSITIONFROMLINE, history - SCROLLBACK, 0);
            sci.call(SCI_DELETERANGE, 0, removed);
            cursor -= removed as usize;
            history = SCROLLBACK;
        }
        sci.call(SCI_SETREADONLY, 1, 0);
        self.history.set(history);

        sci.call(SCI_SETCARETSTYLE, if screen.cursor_visible { CARETSTYLE_BLOCK } else { CARETSTYLE_INVISIBLE }, 0);
        if sci.call(SCI_GETSELECTIONEMPTY, 0, 0) != 0 {
            sci.call(SCI_SETEMPTYSELECTION, cursor, 0);
        }
        if at_bottom {
            let lines = sci.call(SCI_GETLINECOUNT, 0, 0);
            sci.call(SCI_SETFIRSTVISIBLELINE, (lines - on_screen).max(0) as usize, 0);
        }
    }

    /// Tasti che non producono caratteri (frecce, funzione, …) e le combinazioni con Ctrl. true se il tasto è consumato;
    /// gli altri passano da TranslateMessage e arrivano come WM_CHAR.
    pub fn key_down(&self, owner: HWND, key: u16, ctrl: bool, shift: bool, alt: bool) -> bool {
        let modifier = 1 + shift as u8 + 2 * alt as u8 + 4 * ctrl as u8;
        let app_cursor = self.screen.borrow().app_cursor;
        let cursor = |letter: char| -> Vec<u8> {
            match (modifier, app_cursor) {
                (1, true) => format!("\x1bO{letter}").into_bytes(),
                (1, false) => format!("\x1b[{letter}").into_bytes(),
                _ => format!("\x1b[1;{modifier}{letter}").into_bytes(),
            }
        };
        let tilde = |code: u8| -> Vec<u8> { if modifier == 1 { format!("\x1b[{code}~") } else { format!("\x1b[{code};{modifier}~") }.into_bytes() };
        let sequence = match key {
            VK_UP => cursor('A'),
            VK_DOWN => cursor('B'),
            VK_RIGHT => cursor('C'),
            VK_LEFT => cursor('D'),
            VK_HOME => cursor('H'),
            VK_END => cursor('F'),
            VK_INSERT if shift => return self.paste(owner),
            VK_INSERT => tilde(2),
            VK_DELETE => tilde(3),
            VK_PRIOR => tilde(5),
            VK_NEXT => tilde(6),
            VK_F1..=VK_F4 if modifier == 1 => format!("\x1bO{}", (b'P' + (key - VK_F1) as u8) as char).into_bytes(),
            VK_F1..=VK_F4 => format!("\x1b[1;{modifier}{}", (b'P' + (key - VK_F1) as u8) as char).into_bytes(),
            VK_F5..=VK_F12 => tilde([15, 17, 18, 19, 20, 21, 23, 24][(key - VK_F5) as usize]),
            VK_TAB if shift && !ctrl => b"\x1b[Z".to_vec(),
            0x43 if ctrl && !alt && self.sci.call(SCI_GETSELECTIONEMPTY, 0, 0) == 0 => {
                self.sci.call(SCI_COPY, 0, 0);
                self.sci.call(SCI_SETEMPTYSELECTION, self.sci.call(SCI_GETCURRENTPOS, 0, 0) as usize, 0);
                return true;
            }
            0x56 if ctrl && !alt => return self.paste(owner),
            0x41..=0x5A if ctrl && !alt => vec![(key - 0x40) as u8],
            VK_SPACE if ctrl && !alt => vec![0],
            _ => return false,
        };
        self.write(&sequence);
        true
    }

    /// Caratteri da WM_CHAR (`alt` per WM_SYSCHAR): UTF-16 con le coppie surrogate ricomposte.
    pub fn char(&self, unit: u16, alt: bool) {
        let unit = match unit {
            0x08 => 0x7F,
            0x7F => 0x08,
            unit => unit,
        };
        if (0xD800..0xDC00).contains(&unit) {
            self.high_surrogate.set(unit);
            return;
        }
        let units: Vec<u16> = if (0xDC00..0xE000).contains(&unit) { vec![self.high_surrogate.replace(0), unit] } else { vec![unit] };
        let mut bytes = if alt { vec![0x1B] } else { Vec::new() };
        bytes.extend(String::from_utf16_lossy(&units).bytes());
        self.write(&bytes);
    }

    fn paste(&self, owner: HWND) -> bool {
        let Some(text) = clipboard_text(owner) else { return true };
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.screen.borrow().bracketed_paste {
            self.write(format!("\x1b[200~{text}\x1b[201~").as_bytes());
        } else {
            self.write(text.as_bytes());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_xterm_palette() {
        assert_eq!(indexed(16, true), 0);
        assert_eq!(indexed(231, true), 0xFFFFFF);
        assert_eq!(indexed(196, true), bgr(255, 0, 0));
        assert_eq!(indexed(232, true), bgr(8, 8, 8));
        assert_eq!(indexed(1, true), bgr(0xcd, 0x31, 0x31));
    }
}

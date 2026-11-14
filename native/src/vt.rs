//! Emulatore VT minimo per il terminale: schermo e sequenze che ConPTY e le shell usano davvero. Lo scrollback
//! sta nel documento della vista: qui restano solo le righe uscite dallo schermo che la vista non ha ancora preso.
use std::collections::VecDeque;

use vte::{Params, Perform};

/// Righe di scrollback tenute dalla vista; anche le righe in attesa non superano questo numero.
pub const SCROLLBACK: usize = 2000;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct Pen {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cell {
    pub ch: char,
    pub pen: Pen,
}

const DEFAULT_PEN: Pen = Pen { fg: Color::Default, bg: Color::Default, bold: false, underline: false, inverse: false };
const BLANK: Cell = Cell { ch: ' ', pen: DEFAULT_PEN };

pub type Line = Vec<Cell>;

struct Saved {
    row: usize,
    col: usize,
    pen: Pen,
}

pub struct Screen {
    pub cols: usize,
    pub rows: usize,
    pub lines: Vec<Line>,
    /// Righe uscite in cima allo schermo, in attesa che la vista le aggiunga al suo scrollback.
    pub scrollback: VecDeque<Line>,
    /// Sequenza ED 3: la vista deve svuotare il suo scrollback.
    pub clear_scrollback: bool,
    pub row: usize,
    pub col: usize,
    pub cursor_visible: bool,
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    /// Risposte da rimandare alla shell (posizione del cursore, attributi del terminale).
    pub replies: Vec<u8>,
    wrap_pending: bool,
    pen: Pen,
    saved: Saved,
    top: usize,
    bottom: usize,
    /// Schermo principale e cursore mentre è attivo quello alternativo.
    alternate: Option<(Vec<Line>, usize, usize)>,
}

impl Screen {
    pub fn new(cols: usize, rows: usize) -> Screen {
        let (cols, rows) = (cols.max(1), rows.max(1));
        Screen {
            cols,
            rows,
            lines: vec![vec![BLANK; cols]; rows],
            scrollback: VecDeque::new(),
            clear_scrollback: false,
            row: 0,
            col: 0,
            cursor_visible: true,
            app_cursor: false,
            bracketed_paste: false,
            replies: Vec::new(),
            wrap_pending: false,
            pen: DEFAULT_PEN,
            saved: Saved { row: 0, col: 0, pen: DEFAULT_PEN },
            top: 0,
            bottom: rows - 1,
            alternate: None,
        }
    }

    /// Le righe in eccesso sopra il cursore finiscono nello scrollback; ConPTY ridisegna comunque lo schermo dopo il resize.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let (cols, rows) = (cols.max(1), rows.max(1));
        if (cols, rows) == (self.cols, self.rows) {
            return;
        }
        let pushed = self.rows.saturating_sub(rows).min(self.row);
        for line in self.lines.drain(..pushed).collect::<Vec<_>>() {
            self.push_scrollback(line);
        }
        self.row -= pushed;
        self.lines.resize(rows, vec![BLANK; cols]);
        for line in &mut self.lines {
            line.resize(cols, BLANK);
        }
        if let Some((lines, _, _)) = &mut self.alternate {
            lines.resize(rows, vec![BLANK; cols]);
            lines.iter_mut().for_each(|line| line.resize(cols, BLANK));
        }
        self.cols = cols;
        self.rows = rows;
        self.top = 0;
        self.bottom = rows - 1;
        self.row = self.row.min(rows - 1);
        self.col = self.col.min(cols - 1);
        self.wrap_pending = false;
    }

    fn erased(&self) -> Cell {
        Cell { ch: ' ', pen: Pen { bg: self.pen.bg, ..DEFAULT_PEN } }
    }

    fn blank_line(&self) -> Line {
        vec![self.erased(); self.cols]
    }

    fn push_scrollback(&mut self, line: Line) {
        self.scrollback.push_back(line);
        if self.scrollback.len() > SCROLLBACK {
            self.scrollback.pop_front();
        }
    }

    fn scroll_up(&mut self, count: usize) {
        for _ in 0..count.min(self.bottom - self.top + 1) {
            let line = self.lines.remove(self.top);
            self.lines.insert(self.bottom, self.blank_line());
            if self.top == 0 && self.alternate.is_none() {
                self.push_scrollback(line);
            }
        }
    }

    fn scroll_down(&mut self, count: usize) {
        for _ in 0..count.min(self.bottom - self.top + 1) {
            self.lines.remove(self.bottom);
            self.lines.insert(self.top, self.blank_line());
        }
    }

    fn linefeed(&mut self) {
        self.wrap_pending = false;
        if self.row == self.bottom {
            self.scroll_up(1);
        } else if self.row + 1 < self.rows {
            self.row += 1;
        }
    }

    fn reverse_index(&mut self) {
        self.wrap_pending = false;
        if self.row == self.top {
            self.scroll_down(1);
        } else {
            self.row = self.row.saturating_sub(1);
        }
    }

    fn move_to(&mut self, row: usize, col: usize) {
        self.row = row.min(self.rows - 1);
        self.col = col.min(self.cols - 1);
        self.wrap_pending = false;
    }

    fn erase(&mut self, row: usize, from: usize, to: usize) {
        let cell = self.erased();
        let line = &mut self.lines[row];
        let to = to.min(line.len());
        line[from.min(to)..to].fill(cell);
    }

    fn erase_display(&mut self, mode: usize) {
        match mode {
            0 => {
                self.erase(self.row, self.col, self.cols);
                (self.row + 1..self.rows).for_each(|row| self.erase(row, 0, self.cols));
            }
            1 => {
                (0..self.row).for_each(|row| self.erase(row, 0, self.cols));
                self.erase(self.row, 0, self.col + 1);
            }
            2 => (0..self.rows).for_each(|row| self.erase(row, 0, self.cols)),
            3 => {
                self.scrollback.clear();
                self.clear_scrollback = true;
            }
            _ => {}
        }
    }

    fn insert_lines(&mut self, count: usize) {
        if self.row < self.top || self.row > self.bottom {
            return;
        }
        for _ in 0..count.min(self.bottom - self.row + 1) {
            self.lines.remove(self.bottom);
            self.lines.insert(self.row, self.blank_line());
        }
        self.col = 0;
    }

    fn delete_lines(&mut self, count: usize) {
        if self.row < self.top || self.row > self.bottom {
            return;
        }
        for _ in 0..count.min(self.bottom - self.row + 1) {
            self.lines.remove(self.row);
            self.lines.insert(self.bottom, self.blank_line());
        }
        self.col = 0;
    }

    fn set_alternate(&mut self, on: bool) {
        match (on, self.alternate.is_some()) {
            (true, false) => {
                let blank = vec![vec![BLANK; self.cols]; self.rows];
                let main = std::mem::replace(&mut self.lines, blank);
                self.alternate = Some((main, self.row, self.col));
            }
            (false, true) => {
                if let Some((main, row, col)) = self.alternate.take() {
                    self.lines = main;
                    self.move_to(row, col);
                }
            }
            _ => {}
        }
    }

    fn set_mode(&mut self, mode: u16, on: bool) {
        match mode {
            1 => self.app_cursor = on,
            25 => self.cursor_visible = on,
            47 | 1047 | 1049 => self.set_alternate(on),
            2004 => self.bracketed_paste = on,
            _ => {}
        }
    }

    fn sgr(&mut self, params: &Params) {
        if params.is_empty() {
            self.pen = DEFAULT_PEN;
            return;
        }
        let mut iter = params.iter();
        while let Some(param) = iter.next() {
            match param[0] {
                0 => self.pen = DEFAULT_PEN,
                1 => self.pen.bold = true,
                22 => self.pen.bold = false,
                4 => self.pen.underline = true,
                24 => self.pen.underline = false,
                7 => self.pen.inverse = true,
                27 => self.pen.inverse = false,
                code @ 30..=37 => self.pen.fg = Color::Indexed(code as u8 - 30),
                39 => self.pen.fg = Color::Default,
                code @ 40..=47 => self.pen.bg = Color::Indexed(code as u8 - 40),
                49 => self.pen.bg = Color::Default,
                code @ 90..=97 => self.pen.fg = Color::Indexed(code as u8 - 90 + 8),
                code @ 100..=107 => self.pen.bg = Color::Indexed(code as u8 - 100 + 8),
                code @ (38 | 48) => {
                    let color = if param.len() > 1 { extended_color(&param[1..]) } else { extended_color_split(&mut iter) };
                    match (code, color) {
                        (38, Some(color)) => self.pen.fg = color,
                        (48, Some(color)) => self.pen.bg = color,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

/// `38:5:n`, `38:2:r:g:b` e `38:2::r:g:b` (con lo spazio colore vuoto).
fn extended_color(sub: &[u16]) -> Option<Color> {
    match sub {
        [5, index, ..] => Some(Color::Indexed(*index as u8)),
        [2, _, r, g, b, ..] => Some(Color::Rgb(*r as u8, *g as u8, *b as u8)),
        [2, r, g, b] => Some(Color::Rgb(*r as u8, *g as u8, *b as u8)),
        _ => None,
    }
}

/// `38;5;n` e `38;2;r;g;b`: i valori arrivano come parametri separati.
fn extended_color_split<'a>(iter: &mut impl Iterator<Item = &'a [u16]>) -> Option<Color> {
    let mut next = || iter.next().map(|param| param[0]);
    match next()? {
        5 => Some(Color::Indexed(next()? as u8)),
        2 => Some(Color::Rgb(next()? as u8, next()? as u8, next()? as u8)),
        _ => None,
    }
}

/// Il parametro `index` di una sequenza CSI; 0 o assente valgono `default`.
fn arg(params: &Params, index: usize, default: usize) -> usize {
    params.iter().nth(index).map(|param| param[0] as usize).filter(|&value| value != 0).unwrap_or(default)
}

impl Perform for Screen {
    fn print(&mut self, ch: char) {
        if self.wrap_pending {
            self.col = 0;
            self.linefeed();
        }
        // shortcut: i caratteri larghi (CJK, emoji) occupano una cella sola; da gestire se servono.
        self.lines[self.row][self.col] = Cell { ch, pen: self.pen };
        if self.col + 1 == self.cols {
            self.wrap_pending = true;
        } else {
            self.col += 1;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\r' => {
                self.col = 0;
                self.wrap_pending = false;
            }
            b'\n' | 0x0B | 0x0C => self.linefeed(),
            0x08 => {
                self.col = self.col.saturating_sub(1);
                self.wrap_pending = false;
            }
            b'\t' => self.col = ((self.col / 8 + 1) * 8).min(self.cols - 1),
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let private = intermediates.first() == Some(&b'?');
        let n = arg(params, 0, 1);
        let raw = arg(params, 0, 0);
        match action {
            'A' => {
                let floor = if self.row >= self.top { self.top } else { 0 };
                self.move_to(self.row.saturating_sub(n).max(floor), self.col);
            }
            'B' => {
                let ceiling = if self.row <= self.bottom { self.bottom } else { self.rows - 1 };
                self.move_to((self.row + n).min(ceiling), self.col);
            }
            'C' => self.move_to(self.row, self.col + n),
            'D' => self.move_to(self.row, self.col.saturating_sub(n)),
            'E' => self.move_to(self.row + n, 0),
            'F' => self.move_to(self.row.saturating_sub(n), 0),
            'G' | '`' => self.move_to(self.row, n - 1),
            'd' => self.move_to(n - 1, self.col),
            'H' | 'f' => self.move_to(n - 1, arg(params, 1, 1) - 1),
            'J' => self.erase_display(raw),
            'K' => match raw {
                0 => self.erase(self.row, self.col, self.cols),
                1 => self.erase(self.row, 0, self.col + 1),
                _ => self.erase(self.row, 0, self.cols),
            },
            'X' => self.erase(self.row, self.col, self.col + n),
            '@' => {
                let cell = self.erased();
                let line = &mut self.lines[self.row];
                for _ in 0..n.min(self.cols - self.col) {
                    line.pop();
                    line.insert(self.col, cell);
                }
            }
            'P' => {
                let cell = self.erased();
                let line = &mut self.lines[self.row];
                for _ in 0..n.min(self.cols - self.col) {
                    line.remove(self.col);
                    line.push(cell);
                }
            }
            'L' => self.insert_lines(n),
            'M' => self.delete_lines(n),
            'S' => self.scroll_up(n),
            'T' => self.scroll_down(n),
            'm' => self.sgr(params),
            'r' => {
                let bottom = arg(params, 1, self.rows).min(self.rows);
                if n < bottom {
                    self.top = n - 1;
                    self.bottom = bottom - 1;
                    self.move_to(0, 0);
                }
            }
            'h' | 'l' if private => params.iter().for_each(|param| self.set_mode(param[0], action == 'h')),
            'n' if raw == 6 => self.replies.extend(format!("\x1b[{};{}R", self.row + 1, self.col + 1).bytes()),
            'n' if raw == 5 => self.replies.extend(b"\x1b[0n"),
            'c' if !private && intermediates.is_empty() && raw == 0 => self.replies.extend(b"\x1b[?61c"),
            's' if !private => self.saved = Saved { row: self.row, col: self.col, pen: self.pen },
            'u' if !private => {
                self.pen = self.saved.pen;
                self.move_to(self.saved.row, self.saved.col);
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        if !intermediates.is_empty() {
            return;
        }
        match byte {
            b'7' => self.saved = Saved { row: self.row, col: self.col, pen: self.pen },
            b'8' => {
                self.pen = self.saved.pen;
                self.move_to(self.saved.row, self.saved.col);
            }
            b'D' => self.linefeed(),
            b'E' => {
                self.col = 0;
                self.linefeed();
            }
            b'M' => self.reverse_index(),
            b'c' => {
                let scrollback = std::mem::take(&mut self.scrollback);
                *self = Screen { scrollback, ..Screen::new(self.cols, self.rows) };
            }
            _ => {}
        }
    }
}

/// Testo UTF-8 di una riga senza le celle vuote in coda (ne tiene almeno `keep`) e la penna di ogni byte.
pub fn render(line: &[Cell], keep: usize, text: &mut Vec<u8>, pens: &mut Vec<Pen>) {
    let used = line.iter().rposition(|cell| cell.ch != ' ' || cell.pen.bg != Color::Default || cell.pen.inverse).map_or(0, |last| last + 1);
    let mut buffer = [0u8; 4];
    for cell in &line[..used.max(keep).min(line.len())] {
        let bytes = cell.ch.encode_utf8(&mut buffer).as_bytes();
        text.extend_from_slice(bytes);
        pens.extend(std::iter::repeat_n(cell.pen, bytes.len()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(screen: &mut Screen, bytes: &[u8]) {
        vte::Parser::new().advance(screen, bytes);
    }

    fn row_text(line: &[Cell]) -> String {
        let (mut text, mut pens) = (Vec::new(), Vec::new());
        render(line, 0, &mut text, &mut pens);
        String::from_utf8(text).unwrap()
    }

    #[test]
    fn prints_and_moves_to_the_next_line() {
        let mut screen = Screen::new(10, 3);
        feed(&mut screen, b"ab\r\ncd");
        assert_eq!(row_text(&screen.lines[0]), "ab");
        assert_eq!(row_text(&screen.lines[1]), "cd");
        assert_eq!((screen.row, screen.col), (1, 2));
    }

    #[test]
    fn scrolls_lines_into_the_scrollback() {
        let mut screen = Screen::new(10, 2);
        feed(&mut screen, b"1\r\n2\r\n3\r\n4");
        assert_eq!(screen.scrollback.iter().map(|line| row_text(line)).collect::<Vec<_>>(), ["1", "2"]);
        assert_eq!(row_text(&screen.lines[1]), "4");
    }

    #[test]
    fn caps_the_lines_waiting_for_the_view() {
        let mut screen = Screen::new(4, 1);
        for _ in 0..SCROLLBACK + 5 {
            feed(&mut screen, b"x\r\n");
        }
        assert_eq!(screen.scrollback.len(), SCROLLBACK);
    }

    #[test]
    fn wraps_at_the_last_column() {
        let mut screen = Screen::new(3, 2);
        feed(&mut screen, b"abcd");
        assert_eq!(row_text(&screen.lines[0]), "abc");
        assert_eq!(row_text(&screen.lines[1]), "d");
    }

    #[test]
    fn positions_the_cursor_and_erases() {
        let mut screen = Screen::new(10, 3);
        feed(&mut screen, b"hello\r\nworld\x1b[1;3H\x1b[K\x1b[2;1H\x1b[2X");
        assert_eq!(row_text(&screen.lines[0]), "he");
        assert_eq!(row_text(&screen.lines[1]), "  rld");
        feed(&mut screen, b"\x1b[2J");
        assert!(screen.lines.iter().all(|line| row_text(line).is_empty()));
    }

    #[test]
    fn reads_basic_256_and_true_colors() {
        let mut screen = Screen::new(10, 1);
        feed(&mut screen, b"\x1b[1;31ma\x1b[38;5;200mb\x1b[48;2;1;2;3mc\x1b[38:2::4:5:6md\x1b[0me");
        let pens: Vec<Pen> = screen.lines[0][..5].iter().map(|cell| cell.pen).collect();
        assert_eq!(pens[0], Pen { fg: Color::Indexed(1), bold: true, ..DEFAULT_PEN });
        assert_eq!(pens[1].fg, Color::Indexed(200));
        assert_eq!(pens[2].bg, Color::Rgb(1, 2, 3));
        assert_eq!(pens[3].fg, Color::Rgb(4, 5, 6));
        assert_eq!(pens[4], DEFAULT_PEN);
    }

    #[test]
    fn answers_the_cursor_position_request() {
        let mut screen = Screen::new(10, 5);
        feed(&mut screen, b"\x1b[3;4H\x1b[6n");
        assert_eq!(screen.replies, b"\x1b[3;4R");
    }

    #[test]
    fn keeps_the_main_screen_under_the_alternate_one() {
        let mut screen = Screen::new(10, 2);
        feed(&mut screen, b"main\x1b[?1049hfull\x1b[?1049l");
        assert_eq!(row_text(&screen.lines[0]), "main");
        assert_eq!(screen.col, 4);
    }

    #[test]
    fn scrolls_only_inside_the_region() {
        let mut screen = Screen::new(5, 3);
        feed(&mut screen, b"top\x1b[2;3r\x1b[2;1Ha\r\nb\r\nc");
        assert_eq!(row_text(&screen.lines[0]), "top");
        assert_eq!(row_text(&screen.lines[1]), "b");
        assert_eq!(row_text(&screen.lines[2]), "c");
        assert!(screen.scrollback.is_empty());
    }

    #[test]
    fn shrinking_moves_lines_above_the_cursor_to_the_scrollback() {
        let mut screen = Screen::new(5, 3);
        feed(&mut screen, b"1\r\n2\r\n3");
        screen.resize(5, 2);
        assert_eq!(row_text(&screen.scrollback[0]), "1");
        assert_eq!(row_text(&screen.lines[1]), "3");
        assert_eq!(screen.row, 1);
    }

    #[test]
    fn renders_up_to_the_cursor_and_one_pen_per_byte() {
        let line = vec![Cell { ch: 'è', pen: DEFAULT_PEN }, BLANK, BLANK];
        let (mut text, mut pens) = (Vec::new(), Vec::new());
        render(&line, 2, &mut text, &mut pens);
        assert_eq!(text, "è ".as_bytes());
        assert_eq!(pens.len(), 3);
    }
}

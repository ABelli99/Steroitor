//! Diff affiancato su due viste Scintilla: righe allineate, numeri di riga reali nel margine, sfondo per righe
//! aggiunte/modificate/tolte e righe di riempimento, scorrimento sincronizzato. Lo usano la finestra di confronto e
//! l'anteprima della tab Commit.
use std::path::Path;

use crate::diff::{align, Kind, Side};
use crate::lang::{self, Language};
use crate::sci::*;
use crate::theme::Palette;

const SCI_SETREADONLY: u32 = 2171;
const SCI_GETXOFFSET: u32 = 2398;
const SCI_SETXOFFSET: u32 = 2397;
const SC_MARGIN_RTEXT: isize = 5;
const SC_MARK_BACKGROUND: isize = 22;
/// Marker di sfondo: un numero per tipo di riga (Same non ha marker).
pub const MARKERS: [(Kind, usize); 4] = [(Kind::Added, 0), (Kind::Deleted, 1), (Kind::Modified, 2), (Kind::Filler, 3)];
pub const MARKER_CONFLICT: usize = 4;

pub fn blend(color: u32, background: u32, amount: u32) -> u32 {
    let channel = |shift: u32| (((color >> shift) & 0xFF) * amount + ((background >> shift) & 0xFF) * (100 - amount)) / 100;
    channel(0) | (channel(8) << 8) | (channel(16) << 16)
}

/// Colori, margini e marker di una vista del confronto.
pub fn configure(sci: &Sci, palette: &Palette) {
    let opaque = |color: u32| (color | 0xFF00_0000) as isize;
    sci.call(SCI_SETCODEPAGE, SC_CP_UTF8, 0);
    sci.call(SCI_SETSCROLLWIDTH, 1, 0);
    sci.call(SCI_SETSCROLLWIDTHTRACKING, 1, 0);
    sci.call(SCI_SETMARGINLEFT, 0, 4);
    sci.call(SCI_SETMARGINWIDTHN, 1, 0);
    sci.call(SCI_SETMARGINMASKN, 0, 0);
    sci.call(SCI_SETMARGINMASKN, 1, 0);
    sci.call(SCI_SETMARGINBACKN, 0, palette.editor as isize);
    sci.call(SCI_SETMODEVENTMASK, (SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT) as usize, 0);
    sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_CARET, opaque(palette.accent));
    sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_BACK, opaque(palette.selection));
    sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_SELECTION_INACTIVE_BACK, opaque(palette.selection));
    let markers = [(0, palette.git_added), (1, palette.git_ignored), (2, palette.git_modified), (3, palette.border), (MARKER_CONFLICT, palette.danger)];
    for (marker, color) in markers {
        sci.call(SCI_MARKERDEFINE, marker, SC_MARK_BACKGROUND);
        sci.call(SCI_MARKERSETBACK, marker, blend(color, palette.editor, if marker == 3 { 40 } else { 22 }) as isize);
    }
}

/// Testo con l'evidenziazione del linguaggio, senza marker né numeri; resta modificabile.
pub fn set_text(sci: &Sci, text: &str, language: &'static Language, palette: &Palette, font_fraction: isize) {
    lang::set_lexer(sci, language);
    lang::apply_styles(sci, language, palette, font_fraction);
    sci.call(SCI_SETREADONLY, 0, 0);
    sci.call(SCI_MARGINTEXTCLEARALL, 0, 0);
    sci.call(SCI_MARKERDELETEALL, usize::MAX, 0);
    sci.set_text(text.as_bytes());
}

fn show_side(sci: &Sci, side: &Side, language: &'static Language, palette: &Palette, font_fraction: isize) {
    set_text(sci, &side.text, language, palette, font_fraction);
    sci.call(SCI_SETMARGINTYPEN, 0, SC_MARGIN_RTEXT);
    for (line, (number, kind)) in side.numbers.iter().zip(&side.kinds).enumerate() {
        if let Some(number) = number {
            sci.call_str(SCI_MARGINSETTEXT, line, &number.to_string());
            sci.call(SCI_MARGINSETSTYLE, line, STYLE_LINENUMBER as isize);
        }
        if let Some(&(_, marker)) = MARKERS.iter().find(|(marker_kind, _)| marker_kind == kind) {
            sci.call(SCI_MARKERADD, line, marker as isize);
        }
    }
    sci.call(SCI_SETREADONLY, 1, 0);
}

/// Mostra `left` e `right` allineati, in sola lettura; restituisce la prima riga di ogni modifica e il tipo di ogni riga.
pub fn show(panes: [&Sci; 2], left: &str, right: &str, path: &Path, palette: &Palette, font_fraction: isize) -> (Vec<usize>, Vec<Kind>) {
    let language = lang::detect(path);
    let aligned = align(left, right);
    show_side(panes[0], &aligned.left, language, palette, font_fraction);
    show_side(panes[1], &aligned.right, language, palette, font_fraction);
    let kinds = aligned.left.kinds.iter().zip(&aligned.right.kinds).map(|(left, right)| if *left == Kind::Same && *right == Kind::Same { Kind::Same } else { Kind::Modified }).collect();
    (aligned.changes, kinds)
}

const SCI_HIDELINES: u32 = 2227;
const SCI_SHOWLINES: u32 = 2226;
/// Righe uguali tenute attorno a ogni modifica con "Solo le zone cambiate".
const CONTEXT: usize = 3;

/// Righe da nascondere per mostrare solo le zone cambiate: quelle uguali a più di CONTEXT righe da una modifica.
pub fn hidden_lines(kinds: &[Kind]) -> Vec<bool> {
    let mut hidden = vec![true; kinds.len()];
    for (line, kind) in kinds.iter().enumerate() {
        if *kind != Kind::Same {
            for near in line.saturating_sub(CONTEXT)..(line + CONTEXT + 1).min(kinds.len()) {
                hidden[near] = false;
            }
        }
    }
    hidden
}

/// Nasconde (o rimostra) le righe uguali lontane dalle modifiche, sui due lati.
pub fn only_changes(panes: [&Sci; 2], kinds: &[Kind], on: bool) {
    for pane in panes {
        pane.call(SCI_SHOWLINES, 0, kinds.len().saturating_sub(1) as isize);
        if !on {
            continue;
        }
        let hidden = hidden_lines(kinds);
        let mut line = 0;
        while line < hidden.len() {
            if !hidden[line] {
                line += 1;
                continue;
            }
            let start = line;
            while line < hidden.len() && hidden[line] {
                line += 1;
            }
            pane.call(SCI_HIDELINES, start, (line - 1) as isize);
        }
    }
}

/// I due lati hanno le stesse righe: quando uno scorre, l'altro lo segue.
pub fn sync_scroll(panes: [&Sci; 2], source: windows_sys::Win32::Foundation::HWND) {
    let Some(from) = panes.iter().find(|pane| pane.hwnd == source) else { return };
    let (line, x) = (from.call(SCI_GETFIRSTVISIBLELINE, 0, 0), from.call(SCI_GETXOFFSET, 0, 0));
    for other in panes.iter().filter(|pane| pane.hwnd != source) {
        if other.call(SCI_GETFIRSTVISIBLELINE, 0, 0) != line {
            other.call(SCI_SETFIRSTVISIBLELINE, line as usize, 0);
        }
        if other.call(SCI_GETXOFFSET, 0, 0) != x {
            other.call(SCI_SETXOFFSET, x as usize, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_unchanged_lines_far_from_changes() {
        let mut kinds = vec![Kind::Same; 12];
        kinds[6] = Kind::Modified;
        let hidden = hidden_lines(&kinds);
        assert_eq!(hidden.iter().filter(|hidden| !**hidden).count(), 7);
        assert!(hidden[0] && hidden[2] && !hidden[3] && !hidden[9] && hidden[10]);
    }

    #[test]
    fn blends_two_colors() {
        assert_eq!(blend(0x0000FF, 0x000000, 50), 0x00007F);
        assert_eq!(blend(0xFFFFFF, 0x000000, 100), 0xFFFFFF);
    }
}

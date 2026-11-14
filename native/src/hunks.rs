//! Blocchi di righe cambiate tra due versioni di un file (HEAD o index contro l'editor), per le azioni dal gutter:
//! annullare un blocco nell'editor o metterlo in stage da solo.
use std::ops::Range;

use similar::{DiffOp, TextDiff};

#[derive(Debug, PartialEq, Clone)]
pub struct Hunk {
    /// Righe della versione di partenza.
    pub old: Range<usize>,
    /// Righe del testo attuale; vuoto per un blocco tolto, che il gutter segna sulla riga sotto.
    pub new: Range<usize>,
}

fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

fn eol(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Il blocco cambiato che contiene la riga `line` del testo attuale; le terminazioni di riga non contano.
pub fn hunk_at(old: &str, new: &str, line: usize) -> Option<Hunk> {
    let (old_lines, new_lines) = (lines(old), lines(new));
    let diff = TextDiff::configure().timeout(std::time::Duration::from_millis(200)).diff_slices(&old_lines, &new_lines);
    diff.ops().iter().find_map(|op| {
        let hunk = match *op {
            DiffOp::Equal { .. } => return None,
            DiffOp::Insert { old_index, new_index, new_len } => Hunk { old: old_index..old_index, new: new_index..new_index + new_len },
            DiffOp::Delete { old_index, old_len, new_index } => Hunk { old: old_index..old_index + old_len, new: new_index..new_index },
            DiffOp::Replace { old_index, old_len, new_index, new_len } => Hunk { old: old_index..old_index + old_len, new: new_index..new_index + new_len },
        };
        let hit = if hunk.new.is_empty() { line == hunk.new.start } else { hunk.new.contains(&line) };
        hit.then_some(hunk)
    })
}

/// `base` con le righe `range` sostituite dalle righe `from_range` di `from`, nello stile di fine riga di `base`.
pub fn replace_lines(base: &str, range: Range<usize>, from: &str, from_range: Range<usize>) -> String {
    let (base_lines, from_lines) = (lines(base), lines(from));
    let end = range.end.min(base_lines.len());
    let start = range.start.min(end);
    let mut result: Vec<&str> = base_lines[..start].to_vec();
    result.extend(&from_lines[from_range.start.min(from_lines.len())..from_range.end.min(from_lines.len())]);
    result.extend(&base_lines[end..]);
    let eol = eol(base);
    let mut text = result.join(eol);
    if base.ends_with('\n') || (base.is_empty() && from.ends_with('\n')) {
        text.push_str(eol);
    }
    text
}

/// Il testo attuale con il blocco riportato com'era nella versione di partenza.
pub fn rollback(old: &str, new: &str, hunk: &Hunk) -> String {
    replace_lines(new, hunk.new.clone(), old, hunk.old.clone())
}

/// La versione di partenza con il solo blocco preso dal testo attuale (es. il nuovo contenuto dell'index).
pub fn apply(old: &str, new: &str, hunk: &Hunk) -> String {
    replace_lines(old, hunk.old.clone(), new, hunk.new.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "a\nb\nc\nd\ne\n";

    #[test]
    fn finds_the_hunk_under_a_line() {
        let current = "a\r\nB\r\nc\r\nd\r\nnew\r\ne\r\n";
        assert_eq!(hunk_at(HEAD, current, 1), Some(Hunk { old: 1..2, new: 1..2 }));
        assert_eq!(hunk_at(HEAD, current, 4), Some(Hunk { old: 4..4, new: 4..5 }));
        assert_eq!(hunk_at(HEAD, current, 0), None);
        assert_eq!(hunk_at(HEAD, "a\nd\ne\n", 1), Some(Hunk { old: 1..3, new: 1..1 }));
    }

    #[test]
    fn rolls_back_one_hunk_keeping_the_others() {
        let current = "a\r\nB\r\nc\r\nd\r\nnew\r\ne\r\n";
        let hunk = hunk_at(HEAD, current, 4).unwrap();
        assert_eq!(rollback(HEAD, current, &hunk), "a\r\nB\r\nc\r\nd\r\ne\r\n");
        let deleted = "a\nd\ne\n";
        assert_eq!(rollback(HEAD, deleted, &hunk_at(HEAD, deleted, 1).unwrap()), HEAD);
    }

    #[test]
    fn stages_only_one_hunk_in_the_index_style() {
        let current = "a\r\nB\r\nc\r\nd\r\nnew\r\ne\r\n";
        let hunk = hunk_at(HEAD, current, 1).unwrap();
        assert_eq!(apply(HEAD, current, &hunk), "a\nB\nc\nd\ne\n");
        assert_eq!(apply("", "x\ny\n", &hunk_at("", "x\ny\n", 0).unwrap()), "x\ny\n");
        assert_eq!(apply("a\nb", "a\nb\nc", &hunk_at("a\nb", "a\nb\nc", 2).unwrap()), "a\nb\nc");
    }
}

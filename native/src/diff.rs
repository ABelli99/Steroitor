//! Diff a righe allineate per la vista affiancata, e blocchi di conflitto `<<<<<<< ======= >>>>>>>` per il merge.
use similar::{DiffOp, TextDiff};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Same,
    Added,
    Deleted,
    Modified,
    /// Riga vuota inserita per tenere allineato l'altro lato.
    Filler,
}

/// Un lato del diff: testo con le righe di riempimento, numero reale di ogni riga (None per il riempimento) e tipo.
#[derive(Default, Debug)]
pub struct Side {
    pub text: String,
    pub numbers: Vec<Option<usize>>,
    pub kinds: Vec<Kind>,
}

impl Side {
    fn push(&mut self, line: Option<(usize, &str)>, kind: Kind) {
        if !self.kinds.is_empty() {
            self.text.push('\n');
        }
        if let Some((_, text)) = line {
            self.text.push_str(text);
        }
        self.numbers.push(line.map(|(index, _)| index + 1));
        self.kinds.push(if line.is_some() { kind } else { Kind::Filler });
    }
}

pub struct Aligned {
    pub left: Side,
    pub right: Side,
    /// Prima riga allineata di ogni blocco cambiato.
    pub changes: Vec<usize>,
}

fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

pub fn align(left: &str, right: &str) -> Aligned {
    let (old, new) = (lines(left), lines(right));
    let diff = TextDiff::from_slices(&old, &new);
    let mut aligned = Aligned { left: Side::default(), right: Side::default(), changes: Vec::new() };
    for op in diff.ops() {
        let (kind, old_range, new_range) = match *op {
            DiffOp::Equal { old_index, new_index, len } => (Kind::Same, old_index..old_index + len, new_index..new_index + len),
            DiffOp::Delete { old_index, old_len, new_index } => (Kind::Deleted, old_index..old_index + old_len, new_index..new_index),
            DiffOp::Insert { old_index, new_index, new_len } => (Kind::Added, old_index..old_index, new_index..new_index + new_len),
            DiffOp::Replace { old_index, old_len, new_index, new_len } => (Kind::Modified, old_index..old_index + old_len, new_index..new_index + new_len),
        };
        if kind != Kind::Same {
            aligned.changes.push(aligned.left.kinds.len());
        }
        for offset in 0..old_range.len().max(new_range.len()) {
            let left = old_range.clone().nth(offset).map(|index| (index, old[index]));
            let right = new_range.clone().nth(offset).map(|index| (index, new[index]));
            aligned.left.push(left, kind);
            aligned.right.push(right, kind);
        }
    }
    aligned
}

/// Tipo di ogni riga di `text` rispetto a `base` (Same, Added o Modified), per i marker dei lati del merge.
pub fn changed_lines(base: &str, text: &str) -> Vec<Kind> {
    let (old, new) = (lines(base), lines(text));
    let mut kinds = vec![Kind::Same; new.len()];
    for op in TextDiff::from_slices(&old, &new).ops() {
        let (kind, range) = match *op {
            DiffOp::Insert { new_index, new_len, .. } => (Kind::Added, new_index..new_index + new_len),
            DiffOp::Replace { new_index, new_len, .. } => (Kind::Modified, new_index..new_index + new_len),
            _ => continue,
        };
        kinds[range].fill(kind);
    }
    kinds
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Span {
    pub from: usize,
    pub to: usize,
}

/// Blocco `<<<<<<< … ======= … >>>>>>>` (con la sezione base opzionale dello stile diff3), in byte.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Conflict {
    pub from: usize,
    pub to: usize,
    pub ours: Span,
    pub theirs: Span,
}

fn is_marker(line: &str, marker: &str) -> bool {
    line == marker || line.strip_prefix(marker).is_some_and(|rest| rest.starts_with(' '))
}

pub fn parse_conflicts(text: &str) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    // Inizio del blocco, inizio e fine della parte nostra, inizio della loro.
    let mut open: Option<(usize, usize, Option<usize>, Option<usize>)> = None;
    let mut start = 0;
    for raw in text.split_inclusive('\n') {
        let next = start + raw.len();
        let line = raw.trim_end_matches('\n').trim_end_matches('\r');
        if is_marker(line, "<<<<<<<") {
            open = Some((start, next, None, None));
        } else if let Some((from, ours_from, ours_to, theirs_from)) = &mut open {
            if theirs_from.is_none() && is_marker(line, "|||||||") {
                *ours_to = Some(start);
            } else if theirs_from.is_none() && line == "=======" {
                ours_to.get_or_insert(start);
                *theirs_from = Some(next);
            } else if let (Some(theirs), Some(ours_end)) = (*theirs_from, *ours_to) {
                if is_marker(line, ">>>>>>>") {
                    conflicts.push(Conflict { from: *from, to: next, ours: Span { from: *ours_from, to: ours_end }, theirs: Span { from: theirs, to: start } });
                    open = None;
                }
            }
        }
        start = next;
    }
    conflicts
}

/// Risolve tutti i conflitti con la stessa parte, dal fondo per non spostare gli offset.
pub fn resolve_all(text: &str, ours: bool) -> String {
    let mut result = text.to_owned();
    for conflict in parse_conflicts(text).iter().rev() {
        let span = if ours { conflict.ours } else { conflict.theirs };
        let chosen = text[span.from..span.to].to_owned();
        result.replace_range(conflict.from..conflict.to, &chosen);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_changes_with_filler_lines() {
        let aligned = align("a\nb\nc\nd\n", "a\nB\nc\nnew\nd\n");
        assert_eq!(aligned.left.text, "a\nb\nc\n\nd");
        assert_eq!(aligned.right.text, "a\nB\nc\nnew\nd");
        assert_eq!(aligned.left.kinds, [Kind::Same, Kind::Modified, Kind::Same, Kind::Filler, Kind::Same]);
        assert_eq!(aligned.right.kinds[3], Kind::Added);
        assert_eq!(aligned.left.numbers, [Some(1), Some(2), Some(3), None, Some(4)]);
        assert_eq!(aligned.changes, [1, 3]);
    }

    #[test]
    fn pads_the_shorter_side_of_a_replacement() {
        let aligned = align("x\ny\n", "z\n");
        assert_eq!(aligned.right.kinds, [Kind::Modified, Kind::Filler]);
        assert_eq!(aligned.left.text.lines().count(), aligned.right.text.split('\n').count());
    }

    #[test]
    fn marks_lines_changed_from_the_base() {
        assert_eq!(changed_lines("a\nb\n", "a\nB\nc\n"), [Kind::Same, Kind::Modified, Kind::Modified]);
        assert_eq!(changed_lines("a\n", "a\nb\n"), [Kind::Same, Kind::Added]);
    }

    const CONFLICTED: &str = "start\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> topic\nmiddle\n<<<<<<< HEAD\nA\n||||||| base\nB\n=======\nC\n>>>>>>> topic\nend\n";

    #[test]
    fn finds_conflicts_with_and_without_base() {
        let conflicts = parse_conflicts(CONFLICTED);
        assert_eq!(conflicts.len(), 2);
        assert_eq!(&CONFLICTED[conflicts[0].ours.from..conflicts[0].ours.to], "ours\n");
        assert_eq!(&CONFLICTED[conflicts[0].theirs.from..conflicts[0].theirs.to], "theirs\n");
        assert_eq!(&CONFLICTED[conflicts[1].ours.from..conflicts[1].ours.to], "A\n");
        assert_eq!(&CONFLICTED[conflicts[1].theirs.from..conflicts[1].theirs.to], "C\n");
    }

    #[test]
    fn resolves_every_conflict_with_one_side() {
        assert_eq!(resolve_all(CONFLICTED, true), "start\nours\nmiddle\nA\nend\n");
        assert_eq!(resolve_all(CONFLICTED, false), "start\ntheirs\nmiddle\nC\nend\n");
    }

    #[test]
    fn ignores_an_unfinished_block() {
        assert!(parse_conflicts("<<<<<<< HEAD\nx\n=======\ny\n").is_empty());
    }
}

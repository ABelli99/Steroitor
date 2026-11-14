//! Invio come nella versione Tauri (CodeMirror): la riga nuova prende l'indentazione della precedente, un livello in
//! più dopo `{`, `[` o `(`; tra una parentesi aperta e la sua chiusa, la chiusa va su una riga sua.

pub struct NewLine {
    /// Colonne della riga del cursore.
    pub columns: usize,
    /// Colonne della riga in più con la parentesi chiusa, quando serve.
    pub closer: Option<usize>,
}

/// `previous` è la riga dove si è premuto Invio, `rest` quella nuova con il testo spostato dopo il cursore.
pub fn after_enter(previous: &str, previous_columns: usize, rest: &str, unit: usize) -> NewLine {
    let opener = previous.trim_end().chars().last().filter(|ch| matches!(ch, '{' | '[' | '('));
    let Some(opener) = opener else { return NewLine { columns: previous_columns, closer: None } };
    let closer = match opener {
        '{' => '}',
        '[' => ']',
        _ => ')',
    };
    let closes = rest.trim_start().starts_with(closer);
    NewLine { columns: previous_columns + unit, closer: closes.then_some(previous_columns) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_or_adds_one_level() {
        let same = after_enter("    return x;", 4, "", 2);
        assert_eq!((same.columns, same.closer), (4, None));
        let inside = after_enter("  select() {\r", 2, "", 2);
        assert_eq!((inside.columns, inside.closer), (4, None));
        let between = after_enter("  select() {", 2, "  }", 2);
        assert_eq!((between.columns, between.closer), (4, Some(2)));
        let call = after_enter("foo(", 0, "]", 4);
        assert_eq!((call.columns, call.closer), (4, None));
    }
}

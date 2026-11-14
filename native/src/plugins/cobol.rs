//! Plugin COBOL senza server. Controllo del sorgente: colonne del formato fisso, ordine delle DIVISION, IF, EVALUATE,
//! SEARCH e PERFORM in linea chiusi bene, stringhe chiuse, paragrafi e dati usati ma mai dichiarati. Indentazione
//! dopo Invio per aree A e B. Formato fisso, salvo `>>SOURCE FORMAT FREE`, `$SET SOURCEFORMAT"FREE"` o una DIVISION
//! scritta prima della colonna 8.
use std::collections::HashSet;

use super::protocol::Diagnostic;
use crate::lang::{COBOL_CONSTANTS, COBOL_KEYWORDS};

const INDICATOR: usize = 6;
const AREA_A: usize = 7;
const AREA_B: usize = 11;
const CODE_END: usize = 72;
const TAB_STOP: usize = 8;
const DIVISIONS: [&str; 4] = ["IDENTIFICATION", "ENVIRONMENT", "DATA", "PROCEDURE"];
const PROCEDURE: usize = 3;
const INLINE_PERFORM: [&str; 5] = ["UNTIL", "VARYING", "WITH", "TEST", "FOREVER"];
const OPENERS: [&str; 5] = ["IF", "ELSE", "EVALUATE", "WHEN", "SEARCH"];
const PHRASES: [&str; 11] = [
    "AT END", "NOT AT END", "INVALID KEY", "NOT INVALID KEY", "ON EXCEPTION", "NOT ON EXCEPTION", "ON SIZE ERROR",
    "NOT ON SIZE ERROR", "ON OVERFLOW", "NOT ON OVERFLOW", "AT END-OF-PAGE",
];
const LOOKBACK: usize = 200;

type Chars = Vec<(char, usize)>;

/// La riga con le tabulazioni espanse: ogni carattere sta alla sua colonna e porta la colonna UTF-16 nel testo.
fn expand(raw: &str) -> Chars {
    let mut chars = Vec::new();
    let mut utf16 = 0;
    for ch in raw.trim_end_matches('\r').chars() {
        if ch == '\t' {
            let width = TAB_STOP - chars.len() % TAB_STOP;
            chars.extend(std::iter::repeat_n((' ', utf16), width));
        } else {
            chars.push((ch, utf16));
        }
        utf16 += ch.len_utf16();
    }
    chars
}

fn utf16_at(chars: &[(char, usize)], index: usize) -> usize {
    chars.get(index).map_or_else(|| chars.last().map_or(0, |(ch, column)| column + ch.len_utf16()), |(_, column)| *column)
}

fn reserved() -> HashSet<String> {
    COBOL_KEYWORDS.split_whitespace().chain(COBOL_CONSTANTS.split_whitespace()).map(str::to_ascii_uppercase).collect()
}

fn is_free(text: &str) -> bool {
    text.lines().any(|line| {
        let upper = line.to_ascii_uppercase();
        let chars = expand(&upper);
        let Some(first) = chars.iter().position(|(ch, _)| !ch.is_whitespace()) else { return false };
        let content: String = chars[first..].iter().map(|(ch, _)| ch).collect();
        let directive = (content.starts_with(">>SOURCE") || upper.contains("$SET")) && upper.contains("FREE");
        let early_division = first < AREA_A
            && DIVISIONS.iter().chain(&["ID"]).any(|name| content.strip_prefix(name).is_some_and(|rest| rest.starts_with(' ') && rest.trim_start().starts_with("DIVISION")));
        directive || early_division
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Code,
    Continuation,
    Comment,
}

/// Tipo della riga e indici dei caratteri della sua area del codice. Commenti e direttive sono `Comment`.
fn code_area(chars: &[(char, usize)], free: bool) -> (Kind, std::ops::Range<usize>) {
    let (kind, range) = if free {
        (Kind::Code, 0..chars.len())
    } else {
        let kind = match chars.get(INDICATOR).map(|(ch, _)| *ch) {
            Some('*' | '/' | '$') => Kind::Comment,
            Some('-') => Kind::Continuation,
            _ => Kind::Code,
        };
        (kind, AREA_A.min(chars.len())..CODE_END.min(chars.len()))
    };
    let start = chars[range.clone()].iter().position(|(ch, _)| !ch.is_whitespace()).map_or(range.end, |first| range.start + first);
    let begins = |prefix: &str| prefix.chars().enumerate().all(|(offset, wanted)| chars.get(start + offset).is_some_and(|(ch, _)| ch.eq_ignore_ascii_case(&wanted)));
    let directive = begins("*>") || begins(">>") || begins("$SET");
    (if kind == Kind::Code && directive { Kind::Comment } else { kind }, range)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TokenKind {
    Word,
    Number,
    Literal,
    Unclosed,
    Period,
    Other,
}

struct Token {
    /// Le parole in maiuscolo.
    text: String,
    kind: TokenKind,
    line: usize,
    start: usize,
    end: usize,
    /// Colonna con le tabulazioni espanse, per le aree A e B.
    column: usize,
    /// Primo token di una riga di codice che non continua la precedente.
    first: bool,
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '_'
}

/// Fine di una stringa che inizia dopo la virgoletta `from`: (chiusa, indice dopo). La virgoletta doppia è un carattere.
fn scan_string(chars: &[(char, usize)], from: usize, end: usize, quote: char) -> (bool, usize) {
    let mut at = from;
    while at < end {
        if chars[at].0 == quote {
            if at + 1 < end && chars[at + 1].0 == quote {
                at += 2;
                continue;
            }
            return (true, at + 1);
        }
        at += 1;
    }
    (false, end)
}

fn tokenize(lines: &[Chars], free: bool) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    // Stringa arrivata a fine riga: la sua virgoletta e il suo token, finché una riga di continuazione non la chiude.
    let mut open: Option<(char, usize)> = None;
    for (line, chars) in lines.iter().enumerate() {
        let (kind, range) = code_area(chars, free);
        if kind == Kind::Comment || chars[range.clone()].iter().all(|(ch, _)| ch.is_whitespace()) {
            continue;
        }
        let mut at = range.start;
        if let Some((quote, index)) = open.take() {
            match (kind == Kind::Continuation).then(|| (at..range.end).find(|&index| chars[index].0 == quote)).flatten() {
                Some(resume) => {
                    let (closed, next) = scan_string(chars, resume + 1, range.end, quote);
                    if !closed {
                        open = Some((quote, index));
                    }
                    at = next;
                }
                None => tokens[index].kind = TokenKind::Unclosed,
            }
        }
        let mut first = kind == Kind::Code;
        while at < range.end {
            let ch = chars[at].0;
            if ch.is_whitespace() {
                at += 1;
                continue;
            }
            if ch == '*' && chars.get(at + 1).is_some_and(|(next, _)| *next == '>') {
                break;
            }
            let start = at;
            let mut string = |quote: char, from: usize, tokens: &Vec<Token>| {
                let (closed, next) = scan_string(chars, from, range.end, quote);
                if !closed {
                    open = Some((quote, tokens.len()));
                }
                (TokenKind::Literal, next)
            };
            let (token_kind, next) = if ch == '"' || ch == '\'' {
                string(ch, at + 1, &tokens)
            } else if is_word_char(ch) {
                let mut next = at;
                let mut letters = false;
                while next < range.end {
                    let current = chars[next].0;
                    let decimal = current == '.' && !letters && chars.get(next + 1).is_some_and(|(after, _)| after.is_ascii_digit()) && next + 1 < range.end;
                    if !is_word_char(current) && !decimal {
                        break;
                    }
                    letters |= current.is_alphabetic();
                    next += 1;
                }
                let prefix = next - at <= 2 && letters && chars.get(next).is_some_and(|(quote, _)| *quote == '"' || *quote == '\'') && next < range.end;
                if prefix {
                    string(chars[next].0, next + 1, &tokens)
                } else {
                    (if letters { TokenKind::Word } else { TokenKind::Number }, next)
                }
            } else if ch == '.' && (at + 1 >= range.end || chars[at + 1].0.is_whitespace()) {
                (TokenKind::Period, at + 1)
            } else {
                (TokenKind::Other, at + 1)
            };
            let text = chars[start..next].iter().map(|(ch, _)| ch.to_ascii_uppercase()).collect();
            tokens.push(Token { text, kind: token_kind, line, start: chars[start].1, end: utf16_at(chars, next), column: start, first });
            first = false;
            at = next;
        }
    }
    if let Some((_, index)) = open {
        tokens[index].kind = TokenKind::Unclosed;
    }
    tokens
}

fn report(list: &mut Vec<Diagnostic>, token: &Token, error: bool, message: String) {
    list.push(Diagnostic { start: (token.line, token.start), end: (token.line, token.end), message, error });
}

/// Indicatore in colonna 7 e parole tagliate dalla colonna 72, solo nel formato fisso.
fn column_problems(lines: &[Chars], list: &mut Vec<Diagnostic>) {
    for (line, chars) in lines.iter().enumerate() {
        if let Some(&(ch, column)) = chars.get(INDICATOR) {
            if !matches!(ch, ' ' | '*' | '/' | '-' | '$' | 'D' | 'd') {
                let message = format!("«{ch}» non è valido in colonna 7: vanno spazio, *, /, - o D (il codice inizia dalla colonna 8)");
                list.push(Diagnostic { start: (line, column), end: (line, column + ch.len_utf16()), message, error: true });
            }
        }
        if chars.len() > CODE_END && !chars[CODE_END - 1].0.is_whitespace() && !chars[CODE_END].0.is_whitespace() {
            let end = chars[CODE_END..].iter().position(|(ch, _)| ch.is_whitespace()).map_or(chars.len(), |offset| CODE_END + offset);
            let message = "La parola supera la colonna 72: dalla 73 il testo è ignorato".to_string();
            list.push(Diagnostic { start: (line, chars[CODE_END - 1].1), end: (line, utf16_at(chars, end)), message, error: false });
        }
    }
}

/// Divisione di ogni token (indice in DIVISIONS); None prima della prima DIVISION, come in un copybook.
fn divisions(tokens: &[Token], list: &mut Vec<Diagnostic>) -> Vec<Option<usize>> {
    let mut current = None;
    let mut last: Option<usize> = None;
    let mut out = Vec::with_capacity(tokens.len());
    for (index, token) in tokens.iter().enumerate() {
        let name = if token.text == "ID" { "IDENTIFICATION" } else { token.text.as_str() };
        let header = tokens.get(index + 1).is_some_and(|next| next.text == "DIVISION");
        if let Some(division) = DIVISIONS.iter().position(|candidate| *candidate == name).filter(|_| header && token.kind == TokenKind::Word) {
            // Una IDENTIFICATION DIVISION apre un altro programma.
            if division != 0 && last.is_some_and(|last| division <= last) {
                report(list, token, true, format!("{} DIVISION fuori ordine: l'ordine è IDENTIFICATION, ENVIRONMENT, DATA, PROCEDURE", DIVISIONS[division]));
            }
            last = Some(if division == 0 { 0 } else { last.map_or(division, |last| last.max(division)) });
            current = Some(division);
        }
        out.push(current);
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    If,
    Evaluate,
    Search,
    Perform,
}

impl Scope {
    fn name(self) -> &'static str {
        match self {
            Scope::If => "IF",
            Scope::Evaluate => "EVALUATE",
            Scope::Search => "SEARCH",
            Scope::Perform => "PERFORM",
        }
    }

    fn closed_by(word: &str) -> Option<Scope> {
        [Scope::If, Scope::Evaluate, Scope::Search, Scope::Perform].into_iter().find(|scope| word.strip_prefix("END-") == Some(scope.name()))
    }
}

struct Open {
    scope: Scope,
    token: usize,
    otherwise: bool,
}

/// IF, EVALUATE, SEARCH e PERFORM in linea aperti nel periodo corrente.
struct Scopes<'a> {
    tokens: &'a [Token],
    stack: Vec<Open>,
}

impl Scopes<'_> {
    fn unclosed(&self, open: &Open, before: &str, list: &mut Vec<Diagnostic>) {
        let token = &self.tokens[open.token];
        match open.scope {
            Scope::Perform => report(list, token, true, "PERFORM in linea senza END-PERFORM".into()),
            scope => report(list, token, false, format!("{0} senza END-{0} prima di {before}", scope.name())),
        }
    }

    /// Chiude lo scope `wanted` più vicino; quelli aperti sopra restano senza chiusura. false se non c'è.
    fn close(&mut self, wanted: impl Fn(&Open) -> bool, before: &str, list: &mut Vec<Diagnostic>) -> bool {
        let Some(position) = self.stack.iter().rposition(wanted) else { return false };
        for open in self.stack.split_off(position + 1) {
            self.unclosed(&open, before, list);
        }
        true
    }

    /// Il punto chiude tutto; un PERFORM in linea però vuole sempre END-PERFORM.
    fn period(&mut self, list: &mut Vec<Diagnostic>) {
        for open in std::mem::take(&mut self.stack) {
            if open.scope == Scope::Perform {
                self.unclosed(&open, "", list);
            }
        }
    }

    fn word(&mut self, index: usize, word: &str, list: &mut Vec<Diagnostic>) {
        let tokens = self.tokens;
        let token = &tokens[index];
        let push = |stack: &mut Vec<Open>, scope| stack.push(Open { scope, token: index, otherwise: false });
        match word {
            "IF" => push(&mut self.stack, Scope::If),
            "EVALUATE" => push(&mut self.stack, Scope::Evaluate),
            "SEARCH" => push(&mut self.stack, Scope::Search),
            "PERFORM" => push(&mut self.stack, Scope::Perform),
            "ELSE" => {
                // Un ELSE dopo un IF che ha già il suo ELSE chiude quell'IF.
                while self.stack.last().is_some_and(|open| open.scope == Scope::If && open.otherwise) {
                    self.stack.pop();
                }
                if !self.close(|open| open.scope == Scope::If, "ELSE", list) {
                    return report(list, token, true, "ELSE senza IF".into());
                }
                if let Some(open) = self.stack.last_mut() {
                    open.otherwise = true;
                }
            }
            "WHEN" => {
                if !self.close(|open| matches!(open.scope, Scope::Evaluate | Scope::Search), "WHEN", list) {
                    report(list, token, true, "WHEN fuori da EVALUATE o SEARCH".into());
                }
            }
            _ => {
                let Some(scope) = Scope::closed_by(word) else { return };
                if self.close(|open| open.scope == scope, word, list) {
                    self.stack.pop();
                } else {
                    report(list, token, true, format!("{word} senza {}", scope.name()));
                }
            }
        }
    }
}

/// Nomi di paragrafo dopo PERFORM, anche qualificati (OF/IN) e con THRU.
fn procedure_names(tokens: &[Token], mut at: usize) -> Vec<usize> {
    let mut names = Vec::new();
    while tokens.get(at).is_some_and(|token| token.kind == TokenKind::Word) {
        names.push(at);
        match tokens.get(at + 1).map(|token| token.text.as_str()) {
            Some("OF" | "IN" | "THRU" | "THROUGH") => at += 2,
            _ => break,
        }
    }
    names
}

pub fn check(text: &str) -> Vec<Diagnostic> {
    let free = is_free(text);
    let lines: Vec<Chars> = text.split('\n').map(expand).collect();
    let mut list = Vec::new();
    if !free {
        column_problems(&lines, &mut list);
    }
    let tokens = tokenize(&lines, free);
    let reserved = reserved();
    let division = divisions(&tokens, &mut list);
    let is = |index: usize, kind: TokenKind| tokens.get(index).is_some_and(|token| token.kind == kind);
    let in_procedure = |index: usize| division[index] == Some(PROCEDURE);
    let sentence_start = |index: usize| index == 0 || is(index - 1, TokenKind::Period);
    let is_name = |index: usize| is(index, TokenKind::Word) && !reserved.contains(&tokens[index].text);

    for (index, token) in tokens.iter().enumerate() {
        if token.kind == TokenKind::Unclosed && division[index] != Some(0) {
            report(&mut list, token, true, "Stringa non chiusa".into());
        }
    }

    let paragraphs: HashSet<&str> = (0..tokens.len())
        .filter(|&index| in_procedure(index) && sentence_start(index) && is_name(index))
        .filter(|&index| is(index + 1, TokenKind::Period) || tokens.get(index + 1).is_some_and(|next| next.text == "SECTION"))
        .map(|index| tokens[index].text.as_str())
        .collect();

    if !free {
        for (index, token) in tokens.iter().enumerate().filter(|(_, token)| token.first && token.kind != TokenKind::Literal) {
            let next = tokens.get(index + 1).filter(|next| next.line == token.line).map_or("", |next| next.text.as_str());
            let area_a = token.column < AREA_B;
            let header = token.kind == TokenKind::Word && (next == "DIVISION" || next == "SECTION");
            let entry = division[index] == Some(2) && (matches!(token.text.as_str(), "FD" | "SD" | "RD" | "CD") || token.kind == TokenKind::Number && matches!(token.text.trim_start_matches('0'), "1" | "77"));
            if header && !area_a {
                report(&mut list, token, true, format!("{} {next} va nell'area A (colonne 8-11)", token.text));
            } else if entry && !area_a {
                report(&mut list, token, false, format!("{} va nell'area A (colonne 8-11)", token.text));
            } else if in_procedure(index) && !header && area_a && token.kind == TokenKind::Word && reserved.contains(&token.text) && !matches!(token.text.as_str(), "END" | "DECLARATIVES") {
                report(&mut list, token, false, format!("{} è nell'area A: le istruzioni vanno dalla colonna 12", token.text));
            } else if in_procedure(index) && !area_a && paragraphs.contains(token.text.as_str()) && sentence_start(index) {
                report(&mut list, token, false, format!("Il paragrafo {} va nell'area A (colonne 8-11)", token.text));
            }
        }
    }

    let copy = |index: usize| matches!(tokens[index].text.as_str(), "COPY" | "INCLUDE" | "-INC");
    let check_names = !(0..tokens.len()).any(copy);
    let check_paragraphs = !(0..tokens.len()).any(|index| copy(index) && in_procedure(index));
    let declared: HashSet<&str> = (0..tokens.len()).filter(|&index| !in_procedure(index) && is_name(index)).map(|index| tokens[index].text.as_str()).chain(paragraphs.iter().copied()).collect();

    let mut scopes = Scopes { tokens: &tokens, stack: Vec::new() };
    let mut targets = HashSet::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if !in_procedure(index) {
            index += 1;
            continue;
        }
        if token.kind == TokenKind::Period {
            scopes.period(&mut list);
        }
        if token.kind != TokenKind::Word {
            index += 1;
            continue;
        }
        let word = token.text.as_str();
        match word {
            "EXEC" => {
                index = (index..tokens.len()).find(|&at| tokens[at].text == "END-EXEC").unwrap_or(tokens.len());
            }
            "FUNCTION" => index += 1,
            "PERFORM" => {
                let inline = !is_name(index + 1) || tokens.get(index + 2).is_some_and(|after| after.text == "TIMES");
                if inline {
                    scopes.word(index, word, &mut list);
                } else {
                    targets.extend(procedure_names(&tokens, index + 1).into_iter().filter(|&at| !matches!(tokens[at].text.as_str(), "THRU" | "THROUGH")));
                }
            }
            "GO" => {
                let from = index + 1 + usize::from(tokens.get(index + 1).is_some_and(|next| next.text == "TO"));
                targets.extend((from..tokens.len()).take_while(|&at| is_name(at)));
            }
            _ if targets.contains(&index) => {
                if check_paragraphs && !paragraphs.contains(word) {
                    report(&mut list, token, true, format!("Paragrafo o sezione {word} non trovato"));
                }
            }
            _ if reserved.contains(word) => scopes.word(index, word, &mut list),
            _ if check_names && !declared.contains(word) => report(&mut list, token, false, format!("{word} non è dichiarato")),
            _ => {}
        }
        index += 1;
    }
    scopes.period(&mut list);
    list.sort_by_key(|diagnostic| diagnostic.start);
    list
}

/// Colonna della riga `line`, appena creata con Invio, secondo il codice sopra.
pub fn new_line_column(text: &str, line: usize, unit: usize) -> usize {
    let free = is_free(text);
    let lines: Vec<&str> = text.split('\n').collect();
    let code_line = |index: usize| -> Option<(usize, String)> {
        let chars = expand(lines.get(index)?);
        let (kind, range) = code_area(&chars, free);
        if kind == Kind::Comment {
            return None;
        }
        let first = range.start + chars[range.clone()].iter().position(|(ch, _)| !ch.is_whitespace())?;
        let content: String = chars[first..range.end].iter().map(|(ch, _)| ch.to_ascii_uppercase()).collect();
        let content = content.split("*>").next().unwrap_or("").trim_end().to_string();
        (!content.is_empty()).then_some((first, content))
    };
    let above = || (0..line.min(lines.len())).rev().take(LOOKBACK).filter_map(code_line);
    let Some((column, content)) = above().next() else { return if free { 0 } else { AREA_A } };
    let reserved = reserved();
    let in_procedure = lines[..line.min(lines.len())].iter().any(|line| line.to_ascii_uppercase().contains("PROCEDURE DIVISION"));
    let paragraph = |content: &str| {
        let words: Vec<&str> = content.split_whitespace().collect();
        words.len() == 1 && content.ends_with('.') && (!in_procedure || !reserved.contains(content.trim_end_matches('.')))
    };
    let words: Vec<&str> = content.trim_end_matches('.').split_whitespace().collect();
    let sentence_end = content.ends_with('.');
    if words.contains(&"DIVISION") || words.contains(&"SECTION") {
        return column;
    }
    if paragraph(&content) {
        return if free { column + unit } else { AREA_B };
    }
    let first_word = words.first().copied().unwrap_or("");
    let level = first_word.parse::<u8>().is_ok_and(|level| (1..=49).contains(&level));
    if level {
        let group = sentence_end && !words.iter().any(|word| matches!(*word, "PIC" | "PICTURE"));
        return if group { column + unit } else { column };
    }
    let perform = first_word == "PERFORM" && (words.len() == 1 || INLINE_PERFORM.contains(&words[1]) || words.get(2) == Some(&"TIMES"));
    if !sentence_end && (OPENERS.contains(&first_word) || perform || PHRASES.contains(&words.join(" ").as_str())) {
        return column + unit;
    }
    if sentence_end && in_procedure {
        if !free {
            return AREA_B;
        }
        return above().find(|(_, content)| paragraph(content)).map_or(column, |(header, _)| header + unit);
    }
    column
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(lines: &[&str]) -> String {
        lines.iter().map(|line| format!("       {line}\n")).collect()
    }

    fn messages(text: &str) -> Vec<String> {
        check(text).into_iter().map(|diagnostic| format!("{}:{} {}", diagnostic.start.0, diagnostic.start.1, diagnostic.message)).collect()
    }

    const PROGRAM: &[&str] = &[
        "IDENTIFICATION DIVISION.",
        "PROGRAM-ID. HELLO.",
        "DATA DIVISION.",
        "WORKING-STORAGE SECTION.",
        "01 WS-COUNT PIC 9(3) VALUE 0.",
        "01 WS-NAME  PIC X(10) VALUE 'it''s'.",
        "PROCEDURE DIVISION.",
        "MAIN-PARA.",
        "    PERFORM UNTIL WS-COUNT > 3",
        "        IF WS-COUNT = 2",
        "            DISPLAY \"two\"",
        "        ELSE",
        "            DISPLAY WS-NAME",
        "        END-IF",
        "        ADD 1 TO WS-COUNT",
        "    END-PERFORM",
        "    EVALUATE WS-COUNT",
        "        WHEN 4 PERFORM DONE-PARA THRU DONE-EXIT",
        "        WHEN OTHER MOVE FUNCTION UPPER-CASE(WS-NAME) TO WS-NAME",
        "    END-EVALUATE",
        "    STOP RUN.",
        "DONE-PARA.",
        "    DISPLAY X\"41\" WS-COUNT.",
        "DONE-EXIT.",
        "    EXIT.",
    ];

    #[test]
    fn accepts_a_correct_program() {
        assert_eq!(messages(&fixed(PROGRAM)), Vec::<String>::new());
    }

    #[test]
    fn accepts_a_realistic_program_with_sequence_numbers() {
        let source = [
            "IDENTIFICATION DIVISION.",
            "PROGRAM-ID. PAYROLL.",
            "AUTHOR. O'BRIEN.",
            "ENVIRONMENT DIVISION.",
            "CONFIGURATION SECTION.",
            "SPECIAL-NAMES.",
            "    DECIMAL-POINT IS COMMA.",
            "INPUT-OUTPUT SECTION.",
            "FILE-CONTROL.",
            "    SELECT EMP-FILE ASSIGN TO 'EMP.DAT'",
            "        ORGANIZATION IS LINE SEQUENTIAL",
            "        FILE STATUS IS WS-STATUS.",
            "DATA DIVISION.",
            "FILE SECTION.",
            "FD EMP-FILE.",
            "01 EMP-REC.",
            "    05 EMP-ID     PIC 9(5).",
            "    05 EMP-SALARY PIC S9(7)V99 COMP-3.",
            "WORKING-STORAGE SECTION.",
            "01 WS-STATUS PIC XX.",
            "01 WS-EOF    PIC X VALUE 'N'.",
            "    88 END-OF-FILE VALUE 'Y'.",
            "01 WS-TABLE.",
            "    05 WS-ITEM PIC 9 OCCURS 10 INDEXED BY IDX.",
            "01 WS-DATE   PIC 9(8).",
            "01 WS-LINE   PIC X(80).",
            "01 WS-COUNT  PIC 9(3) VALUE ZERO.",
            "PROCEDURE DIVISION.",
            "MAIN SECTION.",
            "START-UP.",
            "    OPEN INPUT EMP-FILE",
            "    ACCEPT WS-DATE FROM DATE YYYYMMDD",
            "    PERFORM UNTIL END-OF-FILE",
            "        READ EMP-FILE",
            "            AT END SET END-OF-FILE TO TRUE",
            "            NOT AT END PERFORM PROCESS-EMP",
            "        END-READ",
            "    END-PERFORM",
            "    PERFORM VARYING IDX FROM 1 BY 1 UNTIL IDX > 10",
            "        MOVE ZERO TO WS-ITEM (IDX)",
            "    END-PERFORM",
            "    EXEC SQL SELECT COUNT(*) INTO :WS-COUNT FROM EMP END-EXEC",
            "    CLOSE EMP-FILE",
            "    DISPLAY 'Totale: ' WS-COUNT UPON CONSOLE",
            "    GOBACK.",
            "PROCESS-EMP.",
            "    STRING EMP-ID DELIMITED BY SIZE ' ' DELIMITED BY SIZE",
            "        INTO WS-LINE",
            "    END-STRING",
            "    INSPECT WS-LINE TALLYING WS-COUNT FOR ALL SPACES",
            "    IF EMP-SALARY > 1000,50 AND WS-LINE(1:2) NOT = SPACES",
            "        CALL 'AUDIT' USING BY REFERENCE EMP-REC",
            "            ON EXCEPTION DISPLAY 'manca AUDIT'",
            "        END-CALL",
            "    END-IF.",
        ];
        let text: String = source.iter().enumerate().map(|(index, line)| format!("{:06} {line}\n", (index + 1) * 100)).collect();
        assert_eq!(messages(&text), Vec::<String>::new());
    }

    #[test]
    fn reports_scopes_names_and_paragraphs() {
        let mut lines = PROGRAM.to_vec();
        lines[13] = "        DISPLAY WS-MISSING";
        lines[15] = "    END-PERFORM.";
        lines[17] = "        WHEN 4 GO TO NOWHERE";
        let found = messages(&fixed(&lines));
        assert!(found.iter().any(|message| message == "9:15 IF senza END-IF prima di END-PERFORM"), "{found:?}");
        assert!(found.iter().any(|message| message == "13:23 WS-MISSING non è dichiarato"), "{found:?}");
        assert!(found.iter().any(|message| message.contains("Paragrafo o sezione NOWHERE non trovato")), "{found:?}");
    }

    #[test]
    fn requires_end_perform_and_matching_terminators() {
        let text = fixed(&["PROCEDURE DIVISION.", "P.", "    PERFORM 3 TIMES", "        DISPLAY 1.", "    END-IF.", "    ELSE.", "    WHEN 1."]);
        let found = messages(&text);
        assert_eq!(
            found,
            vec!["2:11 PERFORM in linea senza END-PERFORM", "4:11 END-IF senza IF", "5:11 ELSE senza IF", "6:11 WHEN fuori da EVALUATE o SEARCH"],
        );
    }

    #[test]
    fn checks_fixed_columns_and_division_order() {
        let mut text = fixed(&["PROCEDURE DIVISION.", "P.", "MOVE 1 TO X.", "    DATA DIVISION."]);
        text.push_str("      XDISPLAY 1.\n");
        text.push_str(&format!("{}{}\n", " ".repeat(11), "A".repeat(70)));
        let found = messages(&text);
        assert!(found.iter().any(|message| message == "3:11 DATA DIVISION fuori ordine: l'ordine è IDENTIFICATION, ENVIRONMENT, DATA, PROCEDURE"), "{found:?}");
        assert!(found.iter().any(|message| message == "3:11 DATA DIVISION va nell'area A (colonne 8-11)"), "{found:?}");
        assert!(found.iter().any(|message| message == "2:7 MOVE è nell'area A: le istruzioni vanno dalla colonna 12"), "{found:?}");
        assert!(found.iter().any(|message| message.starts_with("4:6 «X» non è valido in colonna 7")), "{found:?}");
        assert!(found.iter().any(|message| message.starts_with("5:71 La parola supera la colonna 72")), "{found:?}");
    }

    #[test]
    fn reads_free_format_strings_and_copybooks() {
        let free = ">>SOURCE FORMAT FREE\nPROCEDURE DIVISION.\nP.\n  DISPLAY \"open\n  DISPLAY Y. *> Y viene da un COPY\n  COPY BOOK.\n";
        assert_eq!(messages(free), vec!["3:10 Stringa non chiusa"]);
        let continued = format!("{}      -    \"rest\".\n", fixed(&["PROCEDURE DIVISION.", "P.", "    DISPLAY \"start"]));
        assert_eq!(messages(&continued), Vec::<String>::new());
        assert!(is_free("IDENTIFICATION DIVISION.\n") && !is_free("       IDENTIFICATION DIVISION.\n") && !is_free("\tIDENTIFICATION DIVISION.\n"));
        assert!(messages(&fixed(&["01 A PIC X.", "   05 B PIC X."])).is_empty());
    }

    #[test]
    fn indents_by_area_and_scope() {
        let column = |lines: &[&str]| new_line_column(&format!("{}\n", fixed(lines)), lines.len(), 4);
        assert_eq!(column(&["PROCEDURE DIVISION."]), AREA_A);
        assert_eq!(column(&["PROCEDURE DIVISION.", "MAIN-PARA."]), AREA_B);
        assert_eq!(column(&["PROCEDURE DIVISION.", "P.", "    IF A = 1"]), AREA_B + 4);
        assert_eq!(column(&["PROCEDURE DIVISION.", "P.", "    PERFORM UNTIL A > 1"]), AREA_B + 4);
        assert_eq!(column(&["PROCEDURE DIVISION.", "P.", "    PERFORM OTHER-PARA"]), AREA_B);
        let commented = format!("{}      *   commento\n\n", fixed(&["PROCEDURE DIVISION.", "P.", "        MOVE 1 TO A."]));
        assert_eq!(new_line_column(&commented, 4, 4), AREA_B);
        assert_eq!(column(&["DATA DIVISION.", "FILE-CONTROL."]), AREA_B);
        assert_eq!(column(&["WORKING-STORAGE SECTION.", "01 GROUP-ITEM."]), AREA_A + 4);
        assert_eq!(column(&["WORKING-STORAGE SECTION.", "01 GROUP-ITEM.", "    05 FIELD PIC X."]), AREA_B);
        assert_eq!(new_line_column("", 0, 4), AREA_A);
        let free = ">>SOURCE FORMAT FREE\nPROCEDURE DIVISION.\nMAIN.\n  IF A\n    MOVE 1 TO B.\n";
        assert_eq!(new_line_column(free, 3, 2), 2);
        assert_eq!(new_line_column(free, 4, 2), 4);
        assert_eq!(new_line_column(free, 5, 2), 2);
    }
}

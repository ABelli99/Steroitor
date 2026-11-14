//! HTML dentro JavaScript/TypeScript: JSX (React) e i template inline di Angular (`template: \`…\``) con il
//! loro CSS (`styles`).
//! Il lexer C-like non li conosce; qui si trovano i pezzi da colorare come HTML, che l'editor sovrappone con indicatori.
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Role {
    Tag,
    /// `<`, `</`, `>`, `/>`: col colore del tag, come nella versione Tauri.
    AngleBracket,
    Attribute,
    /// Binding di Angular: `[prop]`, `(event)`, `[(ngModel)]`, `*ngIf`, `#ref`.
    Event,
    Value,
    /// `=`, `{{`, `}}` e gli operatori: colore normale.
    Punctuation,
    /// Testo tra i tag e nomi nelle espressioni: colore normale, così le parole non sembrano keyword.
    Text,
    Keyword,
    Number,
    Function,
    Property,
    Comment,
    /// Classi e pseudo-classi del CSS.
    Type,
}

pub type Span = (Range<usize>, Role);

const MAX_LENGTH: usize = 1 << 20;

fn is_name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'$')
}

/// Salta una stringa o un commento JavaScript che inizia in `at`; None se lì non ne inizia uno.
pub(super) fn skip_js_literal(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes[at] {
        quote @ (b'"' | b'\'' | b'`') => {
            let mut index = at + 1;
            while index < bytes.len() && bytes[index] != quote {
                index += if bytes[index] == b'\\' { 2 } else { 1 };
            }
            Some((index + 1).min(bytes.len()))
        }
        b'/' if bytes.get(at + 1) == Some(&b'/') => Some(bytes[at..].iter().position(|&byte| byte == b'\n').map_or(bytes.len(), |end| at + end)),
        b'/' if bytes.get(at + 1) == Some(&b'*') => Some(find(bytes, at + 2, b"*/").map_or(bytes.len(), |end| end + 2)),
        _ => None,
    }
}

fn find(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    bytes.get(from..)?.windows(needle.len()).position(|window| window == needle).map(|index| from + index)
}

/// Fine (esclusa) del blocco `{ … }` che inizia in `at`, saltando stringhe e commenti.
fn skip_braces(bytes: &[u8], at: usize) -> usize {
    let mut depth = 0;
    let mut index = at;
    while index < bytes.len() {
        if let Some(next) = skip_js_literal(bytes, index) {
            index = next;
            continue;
        }
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
        index += 1;
    }
    bytes.len()
}

fn skip_space(bytes: &[u8], mut index: usize, end: usize) -> usize {
    while index < end && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn name_end(bytes: &[u8], mut index: usize, end: usize) -> usize {
    while index < end && is_name(bytes[index]) {
        index += 1;
    }
    index
}

/// Un elemento JSX che inizia con `<` in `at`; la sua fine, o None se non è JSX valido.
fn jsx_element(bytes: &[u8], at: usize, spans: &mut Vec<Span>) -> Option<usize> {
    let end = bytes.len();
    let mark = spans.len();
    let result = (|| {
        let mut index = at + 1;
        let name = index..name_end(bytes, index, end);
        let fragment = name.is_empty();
        if fragment && bytes.get(index) != Some(&b'>') {
            return None;
        }
        spans.push((at..at + 1, Role::AngleBracket));
        if !fragment {
            spans.push((name.clone(), Role::Tag));
        }
        index = name.end;
        // Attributi, fino a `>` o `/>`.
        loop {
            index = skip_space(bytes, index, end);
            match bytes.get(index)? {
                b'/' if bytes.get(index + 1) == Some(&b'>') => {
                    spans.push((index..index + 2, Role::AngleBracket));
                    return Some(index + 2);
                }
                b'>' => {
                    spans.push((index..index + 1, Role::AngleBracket));
                    index += 1;
                    break;
                }
                b'{' => {
                    let close = skip_braces(bytes, index);
                    jsx_scan(bytes, index + 1..close.saturating_sub(1), spans);
                    index = close;
                }
                byte if is_name(*byte) => {
                    let attribute = index..name_end(bytes, index, end);
                    spans.push((attribute.clone(), Role::Attribute));
                    index = skip_space(bytes, attribute.end, end);
                    if bytes.get(index) == Some(&b'=') {
                        spans.push((index..index + 1, Role::Punctuation));
                        index = skip_space(bytes, index + 1, end);
                        match bytes.get(index)? {
                            b'"' | b'\'' => {
                                let close = skip_js_literal(bytes, index)?;
                                spans.push((index..close, Role::Value));
                                index = close;
                            }
                            b'{' => {
                                let close = skip_braces(bytes, index);
                                jsx_scan(bytes, index + 1..close.saturating_sub(1), spans);
                                index = close;
                            }
                            _ => return None,
                        }
                    }
                }
                _ => return None,
            }
        }
        // Figli, fino al tag di chiusura.
        loop {
            if index >= end {
                return None;
            }
            if bytes[index..].starts_with(b"</") {
                let close_name = index + 2..name_end(bytes, index + 2, end);
                let after = skip_space(bytes, close_name.end, end);
                if bytes.get(after) != Some(&b'>') {
                    return None;
                }
                spans.push((index..index + 2, Role::AngleBracket));
                spans.push((close_name, Role::Tag));
                spans.push((after..after + 1, Role::AngleBracket));
                return Some(after + 1);
            }
            match bytes[index] {
                b'<' => index = jsx_element(bytes, index, spans)?,
                b'{' => {
                    let close = skip_braces(bytes, index);
                    jsx_scan(bytes, index + 1..close.saturating_sub(1), spans);
                    index = close;
                }
                _ => {
                    let start = index;
                    while index < end && bytes[index] != b'<' && bytes[index] != b'{' {
                        index += 1;
                    }
                    spans.push((start..index, Role::Text));
                }
            }
        }
    })();
    if result.is_none() {
        spans.truncate(mark);
    }
    result
}

/// Dove può iniziare un'espressione: dopo `( , = : ? [ { } ; & | ! >`, `return` o `=>`.
fn starts_expression(bytes: &[u8], at: usize) -> bool {
    let before = &bytes[..at];
    let Some(last) = before.iter().rposition(|byte| !byte.is_ascii_whitespace()) else { return true };
    if matches!(bytes[last], b'(' | b',' | b'=' | b':' | b'?' | b'[' | b'{' | b'}' | b';' | b'&' | b'|' | b'!' | b'>') {
        return true;
    }
    let word_start = before[..=last].iter().rposition(|&byte| !(byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$')).map_or(0, |index| index + 1);
    &before[word_start..=last] == b"return"
}

/// Elementi JSX nel tratto `range` di codice JavaScript.
fn jsx_scan(bytes: &[u8], range: Range<usize>, spans: &mut Vec<Span>) {
    let mut index = range.start;
    while index < range.end.min(bytes.len()) {
        if let Some(next) = skip_js_literal(bytes, index) {
            index = next;
            continue;
        }
        let candidate = bytes[index] == b'<' && bytes.get(index + 1).is_some_and(|&next| next.is_ascii_alphabetic() || next == b'>') && starts_expression(bytes, index);
        if candidate {
            if let Some(end) = jsx_element(bytes, index, spans) {
                index = end;
                continue;
            }
        }
        index += 1;
    }
}

const EXPRESSION_KEYWORDS: [&str; 10] = ["this", "typeof", "new", "in", "of", "let", "as", "track", "instanceof", "void"];
const EXPRESSION_ATOMS: [&str; 4] = ["true", "false", "null", "undefined"];
/// Blocchi del control flow di Angular 17+.
const BLOCKS: [&str; 12] = ["if", "else", "for", "switch", "case", "default", "empty", "defer", "placeholder", "loading", "error", "let"];

/// Un'espressione Angular (binding, `{{ }}`, condizione di un blocco) colorata come codice: stringhe, numeri,
/// keyword, funzioni, proprietà dopo il punto, pipe; il resto col colore normale.
fn expression(bytes: &[u8], range: Range<usize>, spans: &mut Vec<Span>) {
    let end = range.end;
    let mut index = range.start;
    while index < end {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
        } else if matches!(byte, b'"' | b'\'' | b'`') {
            let mut close = index + 1;
            while close < end && bytes[close] != byte {
                close += if bytes[close] == b'\\' { 2 } else { 1 };
            }
            let close = (close + 1).min(end);
            spans.push((index..close, Role::Value));
            index = close;
        } else if byte.is_ascii_digit() {
            let start = index;
            while index < end && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'.') {
                index += 1;
            }
            spans.push((start..index, Role::Number));
        } else if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' {
            let start = index;
            while index < end && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_' || bytes[index] == b'$') {
                index += 1;
            }
            let word = std::str::from_utf8(&bytes[start..index]).unwrap_or("");
            let before = bytes[range.start..start].iter().rposition(|byte| !byte.is_ascii_whitespace()).map(|at| bytes[range.start + at]);
            let pipe = before == Some(b'|') && (start < 2 || bytes[range.start..start].iter().rev().filter(|byte| !byte.is_ascii_whitespace()).nth(1) != Some(&b'|'));
            let call = bytes[index..end].iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b'(');
            let role = if EXPRESSION_ATOMS.contains(&word) {
                Role::Number
            } else if EXPRESSION_KEYWORDS.contains(&word) {
                Role::Keyword
            } else if call || pipe {
                Role::Function
            } else if before == Some(b'.') {
                Role::Property
            } else {
                Role::Text
            };
            spans.push((start..index, role));
        } else {
            spans.push((index..index + 1, Role::Punctuation));
            index += 1;
        }
    }
}

/// Binding di Angular: il valore è un'espressione, non una stringa.
fn is_binding(name: &[u8]) -> bool {
    matches!(name.first(), Some(b'[' | b'(' | b'*')) || name.starts_with(b"bind-") || name.starts_with(b"on-")
}

/// HTML di un template Angular nel tratto `range`, con binding, `{{ }}` e blocchi `@if`/`@for` come nella versione Tauri.
fn template_html(bytes: &[u8], range: Range<usize>, spans: &mut Vec<Span>) {
    let end = range.end;
    let mut index = range.start;
    while index < end {
        if bytes[index..end].starts_with(b"<!--") {
            let close = find(&bytes[..end], index + 4, b"-->").map_or(end, |close| close + 3);
            spans.push((index..close, Role::Comment));
            index = close;
        } else if bytes[index] == b'<' && bytes.get(index + 1).is_some_and(|&next| next.is_ascii_alphabetic() || next == b'/') {
            index = template_tag(bytes, index, end, spans);
        } else if bytes[index..end].starts_with(b"{{") {
            let close = find(&bytes[..end], index + 2, b"}}").unwrap_or(end);
            spans.push((index..index + 2, Role::Punctuation));
            expression(bytes, index + 2..close, spans);
            if close < end {
                spans.push((close..close + 2, Role::Punctuation));
            }
            index = (close + 2).min(end);
        } else if bytes[index] == b'@' && bytes.get(index + 1).is_some_and(u8::is_ascii_alphabetic) {
            index = template_block(bytes, index, end, spans);
        } else {
            let start = index;
            index += 1;
            while index < end && bytes[index] != b'<' && bytes[index] != b'@' && !bytes[index..end].starts_with(b"{{") {
                index += 1;
            }
            spans.push((start..index, Role::Text));
        }
    }
}

/// Un tag da `<` in `at`; restituisce dove finisce.
fn template_tag(bytes: &[u8], at: usize, end: usize, spans: &mut Vec<Span>) -> usize {
    let closing = bytes[at + 1] == b'/';
    let name_start = at + 1 + usize::from(closing);
    let name = name_start..name_end(bytes, name_start, end);
    spans.push((at..name_start, Role::AngleBracket));
    spans.push((name.clone(), Role::Tag));
    let mut index = name.end;
    let mut binding = false;
    while index < end && bytes[index] != b'>' {
        match bytes[index] {
            b'"' | b'\'' => {
                let quote = bytes[index];
                let close = bytes[index + 1..end].iter().position(|&byte| byte == quote).map_or(end, |close| index + 1 + close);
                if binding {
                    spans.push((index..index + 1, Role::Value));
                    expression(bytes, index + 1..close, spans);
                    if close < end {
                        spans.push((close..close + 1, Role::Value));
                    }
                } else {
                    spans.push((index..(close + 1).min(end), Role::Value));
                }
                index = (close + 1).min(end);
            }
            b'/' if bytes.get(index + 1) == Some(&b'>') => {
                spans.push((index..index + 1, Role::AngleBracket));
                index += 1;
            }
            b'=' | b'/' => {
                spans.push((index..index + 1, Role::Punctuation));
                index += 1;
            }
            byte if byte.is_ascii_whitespace() => index += 1,
            _ => {
                let start = index;
                while index < end && !bytes[index].is_ascii_whitespace() && !matches!(bytes[index], b'=' | b'>' | b'/' | b'"' | b'\'') {
                    index += 1;
                }
                binding = is_binding(&bytes[start..index]);
                let special = binding || bytes[start] == b'#' || bytes[start] == b'@';
                spans.push((start..index, if special { Role::Event } else { Role::Attribute }));
            }
        }
    }
    if index < end {
        spans.push((index..index + 1, Role::AngleBracket));
        index += 1;
    }
    index
}

/// Un blocco `@if (…) {`, `@for (…)`, `@else`…: keyword e condizione; le graffe restano col colore normale.
fn template_block(bytes: &[u8], at: usize, end: usize, spans: &mut Vec<Span>) -> usize {
    let word_end = (at + 1..end).find(|&index| !bytes[index].is_ascii_alphabetic()).unwrap_or(end);
    if !BLOCKS.iter().any(|block| block.as_bytes() == &bytes[at + 1..word_end]) {
        spans.push((at..word_end, Role::Text));
        return word_end;
    }
    spans.push((at..word_end, Role::Keyword));
    let mut index = skip_space(bytes, word_end, end);
    // `@else if`
    if bytes[index..end].starts_with(b"if") && bytes.get(index + 2).is_some_and(|byte| !byte.is_ascii_alphanumeric()) {
        spans.push((index..index + 2, Role::Keyword));
        index = skip_space(bytes, index + 2, end);
    }
    if bytes.get(index) == Some(&b'(') {
        let mut depth = 0;
        let mut close = index;
        while close < end {
            match bytes[close] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            close += 1;
        }
        spans.push((index..index + 1, Role::Punctuation));
        expression(bytes, index + 1..close, spans);
        if close < end {
            spans.push((close..close + 1, Role::Punctuation));
        }
        index = (close + 1).min(end);
    }
    index
}

/// I template di `template: \`…\`` (decoratore @Component di Angular).
fn angular_scan(bytes: &[u8], spans: &mut Vec<Span>) {
    let mut from = 0;
    while let Some(found) = find(bytes, from, b"template") {
        from = found + 8;
        if found > 0 && is_name(bytes[found - 1]) {
            continue;
        }
        let colon = skip_space(bytes, from, bytes.len());
        if bytes.get(colon) != Some(&b':') {
            continue;
        }
        let tick = skip_space(bytes, colon + 1, bytes.len());
        if bytes.get(tick) != Some(&b'`') {
            continue;
        }
        let close = skip_js_literal(bytes, tick).unwrap_or(bytes.len());
        template_html(bytes, tick + 1..close.saturating_sub(1).max(tick + 1), spans);
        from = close;
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Statement {
    Selector,
    /// Dopo `@media`, `@keyframes`…, fino a `{` o `;`.
    AtRule,
    Declaration,
}

fn css_name_end(bytes: &[u8], mut index: usize, end: usize) -> usize {
    while index < end && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'-' | b'_')) {
        index += 1;
    }
    index
}

/// Selettore o dichiarazione: dentro un blocco decide cosa arriva prima tra `{` e `;`/`}`.
fn statement_at(bytes: &[u8], at: usize, end: usize, depth: usize) -> Statement {
    if depth == 0 {
        return Statement::Selector;
    }
    let first = bytes[at..end].iter().position(|byte| matches!(byte, b'{' | b';' | b'}'));
    if first.is_some_and(|offset| bytes[at + offset] == b'{') {
        Statement::Selector
    } else {
        Statement::Declaration
    }
}

/// CSS di `styles` colorato come il CSS della versione Tauri: tag e classi nei selettori, proprietà, valori come
/// costanti, numeri, funzioni e at-rule come keyword.
fn css(bytes: &[u8], range: Range<usize>, spans: &mut Vec<Span>) {
    let end = range.end;
    let mut index = range.start;
    let mut depth = 0usize;
    let mut statement = None;
    let mut value = false;
    while index < end {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied().filter(|_| index + 1 < end);
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if byte == b'/' && next == Some(b'*') {
            let close = find(&bytes[..end], index + 2, b"*/").map_or(end, |close| close + 2);
            spans.push((index..close, Role::Comment));
            index = close;
            continue;
        }
        if matches!(byte, b'"' | b'\'') {
            let close = bytes[index + 1..end].iter().position(|&candidate| candidate == byte).map_or(end, |close| index + close + 2);
            spans.push((index..close, Role::Value));
            index = close;
            continue;
        }
        if matches!(byte, b'{' | b'}' | b';') {
            depth = match byte {
                b'{' => depth + 1,
                b'}' => depth.saturating_sub(1),
                _ => depth,
            };
            statement = None;
            value = false;
            spans.push((index..index + 1, Role::Punctuation));
            index += 1;
            continue;
        }
        if byte == b'@' {
            let word = css_name_end(bytes, index + 1, end);
            spans.push((index..word, Role::Keyword));
            statement = Some(Statement::AtRule);
            index = word;
            continue;
        }
        let current = *statement.get_or_insert_with(|| statement_at(bytes, index, end, depth));
        let number = byte.is_ascii_digit() || (matches!(byte, b'.' | b'-') && next.is_some_and(|next| next.is_ascii_digit()));
        if number && (value || current != Statement::Declaration) {
            let digits = index + 1 + bytes[index + 1..end].iter().position(|byte| !(byte.is_ascii_digit() || *byte == b'.')).unwrap_or(end - index - 1);
            spans.push((index..digits, Role::Number));
            let unit = css_name_end(bytes, digits, end);
            let unit = if bytes.get(unit) == Some(&b'%') && unit < end { unit + 1 } else { unit };
            spans.push((digits..unit, Role::Text));
            index = unit;
            continue;
        }
        let is_name = byte.is_ascii_alphabetic() || matches!(byte, b'-' | b'_');
        if !is_name {
            let role = match (current, byte) {
                (Statement::Selector, b'.' | b':' | b'#') => {
                    let name_start = (index + 1..end).find(|&at| bytes[at] != b':').unwrap_or(end);
                    let name = name_start..css_name_end(bytes, name_start, end);
                    spans.push((index..name.start, Role::Punctuation));
                    spans.push((name.clone(), if byte == b'#' { Role::Text } else { Role::Type }));
                    index = name.end.max(index + 1);
                    continue;
                }
                (Statement::Selector, b'[') => {
                    let name = css_name_end(bytes, index + 1, end);
                    spans.push((index..index + 1, Role::Punctuation));
                    spans.push((index + 1..name, Role::Attribute));
                    index = name;
                    continue;
                }
                (Statement::Declaration, b':') if !value => {
                    value = true;
                    Role::Punctuation
                }
                (Statement::Declaration, b'!') if value => {
                    let word = css_name_end(bytes, index + 1, end);
                    spans.push((index..word, Role::Keyword));
                    index = word;
                    continue;
                }
                (Statement::Declaration, b'#') if value => {
                    let word = css_name_end(bytes, index + 1, end);
                    spans.push((index..word, Role::Text));
                    index = word;
                    continue;
                }
                _ => Role::Punctuation,
            };
            spans.push((index..index + 1, role));
            index += 1;
            continue;
        }
        let word_end = css_name_end(bytes, index, end);
        let word = &bytes[index..word_end];
        let following = bytes[word_end..end].iter().find(|byte| !byte.is_ascii_whitespace()).copied();
        let role = match current {
            Statement::Selector if word == b"from" || word == b"to" => Role::Keyword,
            Statement::Selector => Role::Tag,
            Statement::AtRule if following == Some(b':') => Role::Property,
            Statement::AtRule => Role::Keyword,
            Statement::Declaration if !value => Role::Property,
            Statement::Declaration if bytes.get(word_end) == Some(&b'(') => Role::Keyword,
            Statement::Declaration if word.starts_with(b"--") => Role::Text,
            Statement::Declaration => Role::Number,
        };
        spans.push((index..word_end, role));
        index = word_end;
    }
}

/// Il CSS di `styles: \`…\`` e `styles: [\`…\`, \`…\`]` nei componenti Angular.
fn styles_scan(bytes: &[u8], spans: &mut Vec<Span>) {
    let mut from = 0;
    while let Some(found) = find(bytes, from, b"styles") {
        from = found + 6;
        if (found > 0 && is_name(bytes[found - 1])) || bytes.get(from).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_') {
            continue;
        }
        let colon = skip_space(bytes, from, bytes.len());
        if bytes.get(colon) != Some(&b':') {
            continue;
        }
        let mut index = skip_space(bytes, colon + 1, bytes.len());
        let list = bytes.get(index) == Some(&b'[');
        if list {
            index += 1;
        }
        loop {
            index = skip_space(bytes, index, bytes.len());
            if bytes.get(index) == Some(&b',') && list {
                index += 1;
                continue;
            }
            if bytes.get(index) != Some(&b'`') {
                break;
            }
            let close = skip_js_literal(bytes, index).unwrap_or(bytes.len());
            css(bytes, index + 1..close.saturating_sub(1).max(index + 1), spans);
            index = close;
            if !list {
                break;
            }
        }
        from = from.max(index);
    }
}

/// I pezzi HTML del testo da colorare; vuoto per file troppo grandi.
pub fn spans(text: &str, react: bool, angular: bool) -> Vec<Span> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    if bytes.len() > MAX_LENGTH {
        return spans;
    }
    if react {
        jsx_scan(bytes, 0..bytes.len(), &mut spans);
    }
    if angular {
        angular_scan(bytes, &mut spans);
        styles_scan(bytes, &mut spans);
    }
    spans.retain(|(range, _)| !range.is_empty());
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(text: &str, react: bool, angular: bool) -> Vec<(&str, Role)> {
        spans(text, react, angular).into_iter().filter(|(_, role)| !matches!(role, Role::Punctuation | Role::AngleBracket)).map(|(range, role)| (&text[range], role)).collect()
    }

    #[test]
    fn colors_jsx_and_leaves_expressions_to_javascript() {
        let code = "const x = a < b;\nreturn (<div className=\"box\" onClick={() => go(<b>x</b>)}>for {name}</div>);";
        assert_eq!(
            roles(code, true, false),
            vec![("div", Role::Tag), ("className", Role::Attribute), ("\"box\"", Role::Value), ("onClick", Role::Attribute), ("b", Role::Tag), ("x", Role::Text), ("b", Role::Tag), ("for ", Role::Text), ("div", Role::Tag)]
        );
    }

    #[test]
    fn ignores_generics_comparisons_and_strings() {
        assert!(roles("useState<string>(''); if (a<b) {} const s = '<div>';", true, false).is_empty());
        assert_eq!(roles("const f = <></>;", true, false), vec![]);
        assert_eq!(spans("const f = <></>;", true, false).len(), 4);
    }

    #[test]
    fn colors_angular_templates_only_when_asked() {
        let code = "@Component({ selector: 'x', template: `<p class=\"a\" *ngIf=\"ok\" (click)=\"go()\">{{ name }} if</p>` })";
        assert_eq!(
            roles(code, false, true),
            vec![
                ("p", Role::Tag),
                ("class", Role::Attribute),
                ("\"a\"", Role::Value),
                ("*ngIf", Role::Event),
                ("\"", Role::Value),
                ("ok", Role::Text),
                ("\"", Role::Value),
                ("(click)", Role::Event),
                ("\"", Role::Value),
                ("go", Role::Function),
                ("\"", Role::Value),
                ("name", Role::Text),
                (" if", Role::Text),
                ("p", Role::Tag),
            ]
        );
        assert!(roles(code, true, false).is_empty());
        assert!(roles("const templateUrl = `<p>`;", false, true).is_empty());
    }

    #[test]
    fn colors_angular_styles_as_css() {
        let code = "@Component({ styles: [`\n  :host { display: block; }\n  .card > h2:hover, a[href] { margin: 0 4px !important; color: #fff; width: calc(100% - 2rem); }\n  @media (max-width: 600px) { .card { padding: .5em; } }\n  /* nota */\n`] })";
        let found = roles(code, false, true);
        for expected in [
            ("host", Role::Type),
            ("display", Role::Property),
            ("block", Role::Number),
            ("card", Role::Type),
            ("h2", Role::Tag),
            ("hover", Role::Type),
            ("a", Role::Tag),
            ("href", Role::Attribute),
            ("margin", Role::Property),
            ("0", Role::Number),
            ("4", Role::Number),
            ("px", Role::Text),
            ("!important", Role::Keyword),
            ("#fff", Role::Text),
            ("calc", Role::Keyword),
            ("@media", Role::Keyword),
            ("max-width", Role::Property),
            ("600", Role::Number),
            ("padding", Role::Property),
            (".5", Role::Number),
            ("/* nota */", Role::Comment),
        ] {
            assert!(found.contains(&expected), "{expected:?} in {found:?}");
        }
        assert!(roles(code, false, false).is_empty());
        assert!(roles("const styles = x;", false, true).is_empty());
    }

    #[test]
    fn colors_angular_expressions_and_blocks_as_code() {
        let code = "template: `@if (user.active && count > 1) {<b [title]=\"'x' + user.name | uppercase\">{{ total(2) }}</b>} @else {no}`";
        let found = roles(code, false, true);
        for expected in [("@if", Role::Keyword), ("active", Role::Property), ("1", Role::Number), ("'x'", Role::Value), ("name", Role::Property), ("uppercase", Role::Function), ("total", Role::Function), ("2", Role::Number), ("@else", Role::Keyword), ("[title]", Role::Event)] {
            assert!(found.contains(&expected), "{expected:?} in {found:?}");
        }
        assert!(spans("template: `<b>`", false, true).iter().any(|(range, role)| *range == (11..12) && *role == Role::AngleBracket));
    }
}

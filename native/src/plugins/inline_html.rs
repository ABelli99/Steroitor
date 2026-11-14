//! HTML dentro JavaScript/TypeScript: JSX (React) e i template inline di Angular (`template: \`…\``).
//! Il lexer C-like non li conosce; qui si trovano i pezzi da colorare come HTML, che l'editor sovrappone con indicatori.
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Role {
    Tag,
    Attribute,
    Value,
    Punctuation,
    /// Testo tra i tag: colore normale, così le parole non sembrano keyword.
    Text,
    /// `{{ … }}` di Angular.
    Binding,
    Comment,
}

pub type Span = (Range<usize>, Role);

const MAX_LENGTH: usize = 1 << 20;

fn is_name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'$')
}

/// Salta una stringa o un commento JavaScript che inizia in `at`; None se lì non ne inizia uno.
fn skip_js_literal(bytes: &[u8], at: usize) -> Option<usize> {
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
        spans.push((at..at + 1, Role::Punctuation));
        if !fragment {
            spans.push((name.clone(), Role::Tag));
        }
        index = name.end;
        // Attributi, fino a `>` o `/>`.
        loop {
            index = skip_space(bytes, index, end);
            match bytes.get(index)? {
                b'/' if bytes.get(index + 1) == Some(&b'>') => {
                    spans.push((index..index + 2, Role::Punctuation));
                    return Some(index + 2);
                }
                b'>' => {
                    spans.push((index..index + 1, Role::Punctuation));
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
                spans.push((index..index + 2, Role::Punctuation));
                spans.push((close_name, Role::Tag));
                spans.push((after..after + 1, Role::Punctuation));
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

/// HTML di un template Angular nel tratto `range`.
fn template_html(bytes: &[u8], range: Range<usize>, spans: &mut Vec<Span>) {
    let end = range.end;
    let mut index = range.start;
    while index < end {
        if bytes[index..end].starts_with(b"<!--") {
            let close = find(&bytes[..end], index + 4, b"-->").map_or(end, |close| close + 3);
            spans.push((index..close, Role::Comment));
            index = close;
        } else if bytes[index] == b'<' && bytes.get(index + 1).is_some_and(|&next| next.is_ascii_alphabetic() || next == b'/') {
            let closing = bytes[index + 1] == b'/';
            let name_start = index + 1 + usize::from(closing);
            let name = name_start..name_end(bytes, name_start, end);
            spans.push((index..name_start, Role::Punctuation));
            spans.push((name.clone(), Role::Tag));
            index = name.end;
            while index < end && bytes[index] != b'>' {
                match bytes[index] {
                    b'"' | b'\'' => {
                        let quote = bytes[index];
                        let close = bytes[index + 1..end].iter().position(|&byte| byte == quote).map_or(end, |close| index + 2 + close);
                        spans.push((index..close, Role::Value));
                        index = close;
                    }
                    b'=' | b'/' => {
                        spans.push((index..index + 1, Role::Punctuation));
                        index += 1;
                    }
                    byte if byte.is_ascii_whitespace() => index += 1,
                    _ => {
                        // Attributi Angular compresi: [prop], (event), [(ngModel)], *ngIf, #ref.
                        let start = index;
                        while index < end && !bytes[index].is_ascii_whitespace() && !matches!(bytes[index], b'=' | b'>' | b'/' | b'"' | b'\'') {
                            index += 1;
                        }
                        spans.push((start..index, Role::Attribute));
                    }
                }
            }
            if index < end {
                spans.push((index..index + 1, Role::Punctuation));
                index += 1;
            }
        } else if bytes[index..end].starts_with(b"{{") {
            let close = find(&bytes[..end], index + 2, b"}}").map_or(end, |close| close + 2);
            spans.push((index..close, Role::Binding));
            index = close;
        } else {
            let start = index;
            while index < end && bytes[index] != b'<' && !bytes[index..end].starts_with(b"{{") {
                index += 1;
            }
            spans.push((start..index, Role::Text));
        }
    }
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
    }
    spans.retain(|(range, _)| !range.is_empty());
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(text: &str, react: bool, angular: bool) -> Vec<(&str, Role)> {
        spans(text, react, angular).into_iter().filter(|(_, role)| *role != Role::Punctuation).map(|(range, role)| (&text[range], role)).collect()
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
        let code = "@Component({ selector: 'x', template: `<p *ngIf=\"ok\" (click)=\"go()\">{{ name }} if</p>` })";
        assert_eq!(
            roles(code, false, true),
            vec![("p", Role::Tag), ("*ngIf", Role::Attribute), ("\"ok\"", Role::Value), ("(click)", Role::Attribute), ("\"go()\"", Role::Value), ("{{ name }}", Role::Binding), (" if", Role::Text), ("p", Role::Tag)]
        );
        assert!(roles(code, true, false).is_empty());
        assert!(roles("const templateUrl = `<p>`;", false, true).is_empty());
    }
}

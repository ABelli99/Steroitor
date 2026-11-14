//! Nomi del codice JavaScript/TypeScript colorati come nella versione Tauri (lezer): chiamate e decoratori come
//! funzioni, `new X`, classi, `implements` e annotazioni di tipo come tipi, membri e chiavi degli oggetti come
//! proprietà. Il lexer C-like conosce solo keyword, stringhe e numeri: l'editor sovrappone questi colori con indicatori.
use std::collections::HashSet;
use std::ops::Range;

use super::inline_html::skip_js_literal;
use crate::lang::{Role, JS_KEYWORDS};

const MAX_LENGTH: usize = 1 << 20;
const MODIFIERS: [&str; 11] = ["private", "public", "protected", "static", "readonly", "override", "declare", "abstract", "async", "get", "set"];
/// Parole che dentro un tipo ne legano due: `class A extends B implements C`, `keyof T`.
const TYPE_LINKS: [&str; 8] = ["extends", "implements", "keyof", "typeof", "readonly", "infer", "is", "unique"];
const GENERIC_LOOKAHEAD: usize = 64;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
    Word,
    Value,
    Punct(u8),
}

pub(super) struct Token {
    pub range: Range<usize>,
    pub kind: Kind,
    pub line_break: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Frame {
    Block,
    Class,
    Object,
    Group,
}

/// Dentro un tipo: profondità delle `<>` e se la virgola lo continua (`implements A, B`).
struct TypeMode {
    start: usize,
    depth: u32,
    lists: bool,
    /// Il nome che segue dichiara una classe o un tipo, con il suo rango (`CLASS`, `DECLARATION`).
    declares: Option<u8>,
}

/// Rango delle dichiarazioni per Vai alla dichiarazione, come nella versione Tauri: vince la classe, poi i membri.
pub const CLASS: u8 = 0;
pub const MEMBER: u8 = 1;
pub const DECLARATION: u8 = 2;
const DECLARING: [&str; 4] = ["const", "let", "var", "function"];

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
}

pub(super) fn tokens(bytes: &[u8]) -> Vec<Token> {
    let mut list = Vec::new();
    let mut index = 0;
    let mut line_break = false;
    while index < bytes.len() {
        let start = index;
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            line_break |= byte == b'\n';
            index += 1;
            continue;
        }
        if let Some(end) = skip_js_literal(bytes, index) {
            index = end;
            if byte == b'/' {
                line_break |= bytes[start..end].contains(&b'\n');
                continue;
            }
        } else if is_word(byte) {
            while index < bytes.len() && (is_word(bytes[index]) || (byte.is_ascii_digit() && bytes[index] == b'.')) {
                index += 1;
            }
        } else {
            index += 1;
        }
        let kind = match byte {
            b'"' | b'\'' | b'`' | b'0'..=b'9' => Kind::Value,
            _ if is_word(byte) => Kind::Word,
            _ => Kind::Punct(byte),
        };
        list.push(Token { range: start..index, kind, line_break });
        line_break = false;
    }
    list
}

/// `<` in `open` apre gli argomenti di tipo di una chiamata: `signal<string>(…)`.
fn generic_call(tokens: &[Token], open: usize) -> bool {
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate().skip(open).take(GENERIC_LOOKAHEAD) {
        match token.kind {
            Kind::Punct(b'<') => depth += 1,
            Kind::Punct(b'>') => {
                depth -= 1;
                if depth == 0 {
                    return tokens.get(index + 1).is_some_and(|next| next.kind == Kind::Punct(b'('));
                }
            }
            Kind::Punct(b',' | b'.' | b'|' | b'&' | b'[' | b']') | Kind::Word | Kind::Value => {}
            Kind::Punct(_) => return false,
        }
    }
    false
}

pub fn spans(text: &str) -> Vec<(Range<usize>, Role)> {
    scan(text).0
}

/// Nomi dichiarati nel testo: classi e tipi, membri delle classi, `const`/`let`/`var`/`function`, con il rango.
pub fn declarations(text: &str) -> Vec<(Range<usize>, u8)> {
    scan(text).1
}

fn scan(text: &str) -> (Vec<(Range<usize>, Role)>, Vec<(Range<usize>, u8)>) {
    let bytes = text.as_bytes();
    if bytes.len() > MAX_LENGTH {
        return (Vec::new(), Vec::new());
    }
    let keywords: HashSet<&str> = JS_KEYWORDS.split(' ').collect();
    let tokens = tokens(bytes);
    let word = |index: usize| tokens.get(index).filter(|token| token.kind == Kind::Word).map(|token| &text[token.range.clone()]);
    let punct = |index: usize| match tokens.get(index)?.kind {
        Kind::Punct(byte) => Some(byte),
        _ => None,
    };
    let mut frames = vec![(Frame::Block, 0u32)];
    let mut types: Option<TypeMode> = None;
    let mut class_header = false;
    let mut alias = false;
    let mut spans = Vec::new();
    let mut declarations = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let previous = index.checked_sub(1);
        let prev_punct = previous.and_then(punct);
        let prev_word = previous.and_then(word);
        let next_punct = punct(index + 1);
        let start_types = |lists, declares| Some(TypeMode { start: index, depth: 0, lists, declares });
        match token.kind {
            Kind::Value => {}
            Kind::Punct(byte) => {
                let in_type = types.is_some();
                if let Some(mode) = types.as_mut() {
                    match byte {
                        b'<' => mode.depth += 1,
                        b'>' if mode.depth > 0 => mode.depth -= 1,
                        b'.' | b'|' | b'&' => {}
                        b'[' if next_punct == Some(b']') => {}
                        b']' if prev_punct == Some(b'[') => {}
                        b',' if mode.depth > 0 || mode.lists => {}
                        _ => types = None,
                    }
                    if types.is_some() {
                        continue;
                    }
                }
                let nested = frames.len() > 1;
                let top = frames.last_mut().expect("il blocco esterno resta");
                match byte {
                    b'{' => {
                        let object = prev_punct.is_some_and(|byte| b"(,=:[?!&|".contains(&byte)) || prev_word.is_some_and(|word| ["return", "yield", "await"].contains(&word));
                        // Un tipo oggetto (`x: { a?: T }`) ha i membri come una classe.
                        let type_literal = in_type && prev_punct.is_some_and(|byte| b":|&<,(".contains(&byte));
                        let frame = if std::mem::take(&mut class_header) || type_literal {
                            Frame::Class
                        } else if object {
                            Frame::Object
                        } else {
                            Frame::Block
                        };
                        frames.push((frame, 0));
                    }
                    b'(' | b'[' => frames.push((Frame::Group, 0)),
                    b'}' | b')' | b']' if nested => {
                        frames.pop();
                    }
                    b'?' if !matches!(next_punct, Some(b'.' | b'?' | b':' | b')' | b',' | b'=')) && prev_punct != Some(b'?') => top.1 += 1,
                    b':' if top.1 > 0 => top.1 -= 1,
                    b':' if top.0 != Frame::Object => {
                        let annotated = prev_word.is_some_and(|word| word != "case" && word != "default") || prev_punct.is_some_and(|byte| b")?!]".contains(&byte));
                        if annotated {
                            types = start_types(false, None);
                        }
                    }
                    b'=' if std::mem::take(&mut alias) => types = start_types(false, None),
                    _ => {}
                }
            }
            Kind::Word => {
                let name = &text[token.range.clone()];
                let keyword = keywords.contains(name);
                if let Some(mode) = &types {
                    let closed = matches!(prev_punct, Some(b'>' | b']'));
                    let joined = mode.depth > 0 || previous == Some(mode.start) || (prev_word.is_none() && !closed) || TYPE_LINKS.contains(&name) || prev_word.is_some_and(|word| TYPE_LINKS.contains(&word));
                    if joined {
                        if !keyword {
                            spans.push((token.range.clone(), Role::Type));
                            if let Some(rank) = mode.declares.filter(|_| previous == Some(mode.start)) {
                                declarations.push((token.range.clone(), rank));
                            }
                        }
                        continue;
                    }
                    types = None;
                }
                let (frame, _) = *frames.last().expect("il blocco esterno resta");
                let after_dot = prev_punct == Some(b'.') && previous.and_then(|at| at.checked_sub(1)).and_then(punct) != Some(b'.');
                let generic = next_punct == Some(b'<') && generic_call(&tokens, index + 1);
                let call = next_punct == Some(b'(') || generic;
                let member_start = match frame {
                    Frame::Class => prev_punct.is_some_and(|byte| b"{;}),".contains(&byte)) || prev_word.is_some_and(|word| MODIFIERS.contains(&word)) || token.line_break,
                    Frame::Object => matches!(prev_punct, Some(b'{' | b',')),
                    _ => false,
                };
                let field = match frame {
                    Frame::Class => matches!(next_punct, Some(b':' | b'?' | b'!' | b'=' | b';')),
                    _ => next_punct == Some(b':'),
                };
                let role = if after_dot {
                    Some(if call { Role::Function } else { Role::Property })
                } else if member_start && (field || (call && !keyword)) {
                    if frame == Frame::Class {
                        declarations.push((token.range.clone(), MEMBER));
                    }
                    Some(Role::Property)
                } else if keyword {
                    match name {
                        "class" | "interface" | "enum" => {
                            class_header = name != "enum";
                            types = start_types(true, Some(if name == "class" { CLASS } else { DECLARATION }));
                        }
                        "extends" | "implements" => types = start_types(true, None),
                        "new" | "as" | "satisfies" | "keyof" => types = start_types(false, None),
                        "type" if word(index + 1).is_some() && (previous.is_none() || prev_punct.is_some_and(|byte| b";{}".contains(&byte)) || prev_word.is_some_and(|word| word == "export" || word == "declare") || token.line_break) => {
                            alias = true;
                            types = start_types(false, Some(DECLARATION));
                        }
                        _ => {}
                    }
                    None
                } else if call {
                    Some(Role::Function)
                } else {
                    None
                };
                if generic {
                    types = start_types(false, None);
                }
                if !after_dot && !keyword && prev_word.is_some_and(|word| DECLARING.contains(&word)) {
                    declarations.push((token.range.clone(), DECLARATION));
                }
                if let Some(role) = role {
                    spans.push((token.range.clone(), role));
                }
            }
        }
    }
    (spans, declarations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(text: &str) -> Vec<(&str, Role)> {
        spans(text).into_iter().map(|(range, role)| (&text[range], role)).collect()
    }

    fn has(found: &[(&str, Role)], name: &str, role: Role) -> bool {
        found.contains(&(name, role))
    }

    #[test]
    fn colors_an_angular_component_like_lezer() {
        let code = "import { Component, EventEmitter, Input, OnChanges, Output, inject } from '@angular/core';\n\
            @Component({ selector: 'app-x', standalone: true, imports: [CommonModule], template: `<p>{{ a ? b : c }}</p>` })\n\
            export class ButtonComponent implements OnChanges, OnInit {\n\
              @Input() label: string = '';\n\
              @Output() clicked = new EventEmitter<string>();\n\
              private readonly http = inject(HttpClient);\n\
              count = signal<number>(0);\n\
              el!: ElementRef<HTMLElement>\n\
              opts: { hidden?: Selector<any>, size: Size } | undefined;\n\
              ngOnChanges(changes: SimpleChanges): void {\n\
                const ok = this.label ? changes.x : null;\n\
                this.clicked.emit(ok);\n\
              }\n\
            }";
        let found = roles(code);
        for (name, role) in [
            ("Component", Role::Function),
            ("selector", Role::Property),
            ("standalone", Role::Property),
            ("imports", Role::Property),
            ("template", Role::Property),
            ("ButtonComponent", Role::Type),
            ("OnChanges", Role::Type),
            ("OnInit", Role::Type),
            ("Input", Role::Function),
            ("label", Role::Property),
            ("Output", Role::Function),
            ("clicked", Role::Property),
            ("EventEmitter", Role::Type),
            ("http", Role::Property),
            ("inject", Role::Function),
            ("count", Role::Property),
            ("signal", Role::Function),
            ("el", Role::Property),
            ("ElementRef", Role::Type),
            ("HTMLElement", Role::Type),
            ("hidden", Role::Property),
            ("Selector", Role::Type),
            ("size", Role::Property),
            ("Size", Role::Type),
            ("ngOnChanges", Role::Property),
            ("SimpleChanges", Role::Type),
            ("x", Role::Property),
            ("emit", Role::Function),
        ] {
            assert!(has(&found, name, role), "{name} in {:?}", found);
        }
        for plain in ["CommonModule", "HttpClient", "changes", "ok", "a", "b"] {
            assert!(!found.iter().any(|(text, _)| *text == plain), "{plain} non va colorato: {found:?}");
        }
    }

    #[test]
    fn leaves_comparisons_ternaries_and_destructuring_alone() {
        let found = roles("const n = a < b ? c : d; const { e, f } = g; if (h > 1) { i(); }\nlet j: Map<string, K> = new Map();\nconst o = { p: q ? r : s };");
        assert_eq!(found.iter().filter(|(_, role)| *role == Role::Function).map(|(text, _)| *text).collect::<Vec<_>>(), vec!["i"]);
        assert!(has(&found, "Map", Role::Type) && has(&found, "K", Role::Type) && has(&found, "p", Role::Property));
        for plain in ["a", "b", "c", "d", "e", "g", "h", "q", "r", "s"] {
            assert!(!found.iter().any(|(text, _)| *text == plain), "{plain}: {found:?}");
        }
    }
}

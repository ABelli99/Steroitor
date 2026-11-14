//! Ctrl+click e Ctrl+B senza server, come nella versione Tauri: il simbolo porta alla sua dichiarazione nel file,
//! un nome importato al file del modulo, un tag `app-…` al componente con quel `selector`. Nei `.component.html`
//! i nomi si cercano nel `.component.ts` accanto.
use std::ops::Range;
use std::path::{Path, PathBuf};

use super::position_of;
use super::script_names::{declarations, tokens, Kind, CLASS, DECLARATION};

const SCRIPT_EXTENSIONS: [&str; 6] = ["ts", "tsx", "js", "jsx", "mts", "mjs"];

pub enum Jump {
    Here(usize),
    /// File e (riga, colonna UTF-16).
    File(PathBuf, (usize, usize)),
}

enum Definition {
    Local(usize),
    Import { source: String, name: String },
}

struct Import {
    local: Range<usize>,
    name: String,
    source: String,
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
}

/// Identificatore sotto `pos`, o nome di tag con i trattini (`app-button`) se è dopo `<` o `</`.
pub fn target_at(text: &str, pos: usize) -> Option<(Range<usize>, bool)> {
    let bytes = text.as_bytes();
    let pos = pos.min(bytes.len());
    let line_start = bytes[..pos].iter().rposition(|&byte| byte == b'\n').map_or(0, |index| index + 1);
    let line_end = bytes[pos..].iter().position(|&byte| byte == b'\n' || byte == b'\r').map_or(bytes.len(), |index| pos + index);
    let around = |accept: &dyn Fn(u8) -> bool| {
        let from = (line_start..pos).rev().take_while(|&index| accept(bytes[index])).last().unwrap_or(pos);
        let to = (pos..line_end).find(|&index| !accept(bytes[index])).unwrap_or(line_end);
        from..to
    };
    let tag_range = around(&|byte| is_word(byte) || byte == b'-');
    let before = &text[line_start..tag_range.start];
    let tag = before.ends_with('<') || before.ends_with("</");
    let range = if tag { tag_range } else { around(&is_word) };
    let first = *bytes.get(range.start)?;
    (!range.is_empty() && (first.is_ascii_alphabetic() || first == b'_' || first == b'$' || first >= 0x80)).then_some((range, tag))
}

/// `import { A, B as C } from 'x'`, `import D from 'x'`, `import * as E from 'x'`.
fn imports(text: &str) -> Vec<Import> {
    let tokens = tokens(text.as_bytes());
    let word = |index: usize| tokens.get(index).filter(|token| token.kind == Kind::Word).map(|token| &text[token.range.clone()]);
    let mut found = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let statement = index == 0 || token.line_break || matches!(tokens[index - 1].kind, Kind::Punct(b';' | b'}'));
        if word(index) != Some("import") || !statement || matches!(tokens.get(index + 1).map(|token| token.kind), Some(Kind::Punct(b'(' | b'.'))) {
            continue;
        }
        let mut names: Vec<(Range<usize>, String)> = Vec::new();
        let mut source = None;
        let mut at = index + 1;
        while let Some(token) = tokens.get(at) {
            match (token.kind, word(at)) {
                (Kind::Value, _) => {
                    source = Some(text[token.range.clone()].trim_matches(|ch| matches!(ch, '\'' | '"' | '`')).to_string());
                    break;
                }
                (Kind::Punct(b';'), _) => break,
                (Kind::Word, Some("from" | "type")) => {}
                (Kind::Word, Some(name)) if at > 0 && word(at - 1) == Some("as") => {
                    let original = match tokens[at - 2].kind {
                        Kind::Word => {
                            names.pop();
                            text[tokens[at - 2].range.clone()].to_string()
                        }
                        _ => name.to_string(),
                    };
                    names.push((token.range.clone(), original));
                }
                (Kind::Word, Some("as")) => {}
                (Kind::Word, Some(name)) => names.push((token.range.clone(), name.to_string())),
                _ => {}
            }
            at += 1;
        }
        let Some(source) = source else { continue };
        found.extend(names.into_iter().map(|(local, name)| Import { local, name, source: source.clone() }));
    }
    found
}

/// Dove è dichiarato `name`: la classe vince sui membri, i membri sulle variabili, queste sugli import.
/// None se `at` è già la dichiarazione.
fn definition(text: &str, name: &str, at: Option<&Range<usize>>) -> Option<Definition> {
    let best = declarations(text).into_iter().filter(|(range, _)| &text[range.clone()] == name).min_by_key(|(range, rank)| (*rank, range.start));
    if let Some((range, _)) = best {
        let itself = at.is_some_and(|at| range.start <= at.start && range.end >= at.end);
        return (!itself).then_some(Definition::Local(range.start));
    }
    let import = imports(text).into_iter().find(|import| &text[import.local.clone()] == name)?;
    Some(Definition::Import { source: import.source, name: import.name })
}

/// Il file di un import relativo, o di un alias (`@app/…`, `src/…`) cercato per suffisso nel progetto.
fn resolve_import(from_file: &Path, source: &str, root: &Path) -> Option<PathBuf> {
    let candidates = |base: &str| {
        let stem = base.trim_end_matches(".js").trim_end_matches(".jsx").trim_end_matches(".mjs").to_string();
        let mut list = vec![base.to_string()];
        list.extend(SCRIPT_EXTENSIONS.iter().map(|extension| format!("{stem}.{extension}")));
        list.extend(SCRIPT_EXTENSIONS.iter().map(|extension| format!("{base}/index.{extension}")));
        list
    };
    if source.starts_with('.') {
        let base = lexical(&from_file.parent()?.join(source));
        return candidates(&base.to_string_lossy()).into_iter().map(PathBuf::from).find(|path| path.is_file());
    }
    let normalize = |path: &str| path.replace('\\', "/").to_lowercase();
    let files: Vec<(String, String)> = steroitor_core::fs::list_files(root.to_string_lossy().into_owned()).into_iter().map(|file| (normalize(&file), file)).collect();
    let segments: Vec<String> = source.to_lowercase().split('/').map(str::to_string).collect();
    let suffixes = [segments.join("/"), segments[1..].join("/")];
    for suffix in suffixes.iter().filter(|suffix| !suffix.is_empty()) {
        for candidate in candidates(suffix) {
            let wanted = format!("/{candidate}");
            if let Some((_, file)) = files.iter().find(|(file, _)| file.ends_with(&wanted) && !file.contains("/node_modules/")) {
                return Some(PathBuf::from(file));
            }
        }
    }
    None
}

/// Il percorso senza `.` e `..`, senza toccare il disco: canonicalize su Windows aggiunge `\\?\`.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Offset della dichiarazione di `name` in un modulo: classe, tipo, funzione o variabile, poi `export { … }`.
fn declaration_offset(content: &str, name: &str) -> usize {
    let declared = declarations(content).into_iter().filter(|(range, rank)| (*rank == CLASS || *rank == DECLARATION) && &content[range.clone()] == name).map(|(range, _)| range.start).min();
    if let Some(offset) = declared {
        return offset;
    }
    let mut start = 0;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let exported = trimmed.starts_with("export") && trimmed.contains('{');
        let word_at = line.match_indices(name).map(|(index, _)| index).find(|&index| {
            let before = line[..index].bytes().last().is_none_or(|byte| !is_word(byte));
            let after = line[index + name.len()..].bytes().next().is_none_or(|byte| !is_word(byte));
            before && after
        });
        if name == "default" && trimmed.starts_with("export default") {
            return start + line.len() - trimmed.len();
        }
        if let (true, Some(index)) = (exported, word_at) {
            return start + index;
        }
        start += line.len();
    }
    0
}

/// Vero se la riga dichiara il `selector` di un componente che corrisponde al tag.
fn declares_selector(line: &str, selector: &str) -> bool {
    let Some(at) = line.find("selector") else { return false };
    let rest = line[at + "selector".len()..].trim_start();
    let Some(rest) = rest.strip_prefix(':').map(str::trim_start) else { return false };
    let Some(quote) = rest.chars().next().filter(|ch| matches!(ch, '\'' | '"' | '`')) else { return false };
    let Some(value) = rest[1..].split(quote).next() else { return false };
    value.split(',').any(|part| part.trim() == selector)
}

fn find_selector(root: &Path, selector: &str) -> Option<Jump> {
    let matches = steroitor_core::fs::search_files(root.to_string_lossy().into_owned(), selector.to_string(), vec!["ts".into(), "js".into()]);
    let found = matches.into_iter().find(|candidate| declares_selector(&candidate.text, selector))?;
    let column = found.text.find(selector).map_or(0, |index| found.text[..index].encode_utf16().count());
    Some(Jump::File(PathBuf::from(found.path), (found.line.saturating_sub(1), column)))
}

fn in_file(path: &Path, definition: Definition, root: &Path) -> Option<Jump> {
    match definition {
        Definition::Local(offset) => {
            let content = std::fs::read_to_string(path).ok()?;
            Some(Jump::File(path.to_path_buf(), position_of(&content, offset)))
        }
        Definition::Import { source, name } => {
            let file = resolve_import(path, &source, root)?;
            let content = std::fs::read_to_string(&file).unwrap_or_default();
            Some(Jump::File(file, position_of(&content, declaration_offset(&content, &name))))
        }
    }
}

/// Dove porta il simbolo sotto `pos` del file `path` (testo `text`, anche non salvato).
pub fn jump(path: &Path, text: &str, pos: usize, root: &Path) -> Option<Jump> {
    let (range, tag) = target_at(text, pos)?;
    let name = &text[range.clone()];
    if tag && name.contains('-') {
        return find_selector(root, name);
    }
    let html = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm"));
    if html {
        let script = path.with_extension("ts");
        let content = std::fs::read_to_string(&script).ok()?;
        let found = definition(&content, name, None)?;
        return in_file(&script, found, root);
    }
    match definition(text, name, Some(&range))? {
        Definition::Local(offset) => Some(Jump::Here(offset)),
        other => in_file(path, other, root),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPONENT: &str = "import { Component, EventEmitter as Emitter } from '@angular/core';\n\
        import Default from './other';\n\
        import { UserCard } from '../user/user-card.component';\n\
        @Component({ selector: 'app-list', template: `<app-user-card (click)=\"select(item)\"></app-user-card>` })\n\
        export class ListComponent {\n\
          selected = 0;\n\
          select(item: number) { this.selected = item; }\n\
        }\n\
        const selected = 1;";

    fn local(text: &str, name: &str, occurrence: usize) -> Option<usize> {
        let at = text.match_indices(name).nth(occurrence)?.0;
        let (range, _) = target_at(text, at + 1)?;
        match definition(text, &text[range.clone()], Some(&range))? {
            Definition::Local(offset) => Some(offset),
            _ => None,
        }
    }

    #[test]
    fn finds_members_before_variables_and_skips_the_declaration_itself() {
        let text = COMPONENT;
        assert_eq!(local(text, "selected", 1), text.find("selected = 0"));
        assert_eq!(local(text, "select(", 0), text.find("select(item: number"));
        assert_eq!(local(text, "selected = 0", 0), None);
    }

    #[test]
    fn reads_imports_with_aliases() {
        let found: Vec<(String, String, String)> = imports(COMPONENT).into_iter().map(|import| (COMPONENT[import.local].to_string(), import.name, import.source)).collect();
        assert_eq!(
            found,
            vec![
                ("Component".into(), "Component".into(), "@angular/core".into()),
                ("Emitter".into(), "EventEmitter".into(), "@angular/core".into()),
                ("Default".into(), "Default".into(), "./other".into()),
                ("UserCard".into(), "UserCard".into(), "../user/user-card.component".into()),
            ]
        );
    }

    #[test]
    fn recognizes_tags_and_selectors() {
        let text = COMPONENT;
        let at = text.find("app-user-card").unwrap() + 5;
        let (range, tag) = target_at(text, at).unwrap();
        assert!(tag && &text[range] == "app-user-card");
        assert!(!target_at(text, text.find("item)").unwrap()).unwrap().1);
        assert!(declares_selector("  selector: 'app-user-card',", "app-user-card"));
        assert!(declares_selector("selector: \"a-b, app-x\"", "app-x"));
        assert!(!declares_selector("  selector: 'app-user-card-large',", "app-user-card"));
    }

    #[test]
    fn jumps_into_imported_files_and_components() {
        let dir = std::env::temp_dir().join(format!("steroitor-navigation-{}", std::process::id()));
        let user = dir.join("src/app/user");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(dir.join("src/app/list")).unwrap();
        let card = "import { Component } from '@angular/core';\n@Component({\n  selector: 'app-user-card',\n})\nexport class UserCard {}\n";
        std::fs::write(user.join("user-card.component.ts"), card).unwrap();
        let path = dir.join("src/app/list/list.component.ts");
        std::fs::write(&path, COMPONENT).unwrap();
        let html = dir.join("src/app/list/list.component.html");
        std::fs::write(&html, "<p (click)=\"select(1)\">x</p>").unwrap();

        let at = COMPONENT.find("UserCard }").unwrap();
        let Some(Jump::File(file, position)) = jump(&path, COMPONENT, at, &dir) else { panic!("import") };
        assert!(file == user.join("user-card.component.ts") && position == (4, 13), "{file:?} {position:?}");

        let at = COMPONENT.find("app-user-card").unwrap() + 2;
        let Some(Jump::File(file, position)) = jump(&path, COMPONENT, at, &dir) else { panic!("selector") };
        assert!(file.ends_with("user-card.component.ts") && position == (2, 13), "{file:?} {position:?}");

        let Some(Jump::File(file, position)) = jump(&html, "<p (click)=\"select(1)\">x</p>", 13, &dir) else { panic!("html") };
        assert!(file.ends_with("list.component.ts") && position.0 == 6, "{file:?} {position:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_exported_declarations() {
        let module = "const a = 1;\nexport function helper() {}\nexport { a as b, other };\nexport default class {}";
        assert_eq!(declaration_offset(module, "helper"), module.find("helper").unwrap());
        assert_eq!(declaration_offset(module, "other"), module.find("other").unwrap());
        assert_eq!(declaration_offset(module, "default"), module.find("export default").unwrap());
        assert_eq!(declaration_offset(module, "missing"), 0);
    }
}

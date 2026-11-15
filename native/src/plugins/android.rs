//! Risorse Android senza server: `R.<tipo>.<nome>` in Kotlin e Java e `@<tipo>/<nome>` negli XML delle risorse e nel
//! manifest, completati e controllati con le risorse di tutti i moduli del progetto (cartelle `src/<set>/res`).
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use super::jvm_source::{is_ident, mask};
use super::protocol::Diagnostic;
use super::{position_of, Local};

const SKIP_DIRS: [&str; 5] = ["build", "node_modules", "gradle", "intermediates", "generated"];
const MAX_DEPTH: usize = 6;
/// Tipi controllati: stili, attributi e styleable vengono spesso dalle librerie (AppCompat, Material) e non si controllano.
const CHECKED: [&str; 21] = [
    "anim", "animator", "array", "bool", "color", "dimen", "drawable", "font", "fraction", "id", "integer", "interpolator", "layout", "menu", "mipmap", "navigation", "plurals", "raw", "string", "transition", "xml",
];

/// Nomi delle risorse per tipo, come scritti negli XML (gli stili con i punti).
pub type Resources = BTreeMap<String, BTreeSet<String>>;

/// Come il nome compare in R: i punti degli stili diventano trattini bassi.
fn r_name(name: &str) -> String {
    name.replace('.', "_")
}

/// La cartella del progetto (quella con settings.gradle, altrimenti il modulo) se il file è Kotlin o Java di un modulo
/// Android, un XML delle sue risorse o il suo manifest.
pub fn project(path: &Path) -> Option<PathBuf> {
    let extension = path.extension()?.to_string_lossy().to_lowercase();
    let xml = extension == "xml";
    if xml {
        let manifest = path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("AndroidManifest.xml"));
        let resource = path.parent().and_then(Path::parent).and_then(Path::file_name).is_some_and(|name| name == "res");
        if !manifest && !resource {
            return None;
        }
    } else if extension != "kt" && extension != "java" {
        return None;
    }
    let module = path.ancestors().skip(1).find(|dir| dir.join("src").join("main").join("AndroidManifest.xml").is_file())?;
    let settings = module.ancestors().find(|dir| ["settings.gradle", "settings.gradle.kts"].iter().any(|name| dir.join(name).is_file()));
    Some(settings.unwrap_or(module).to_path_buf())
}

/// shortcut: rilegge le risorse a ogni controllo e completamento; con progetti molto grandi conviene una cache per cartella.
pub fn index(project: &Path) -> Resources {
    let mut resources = Resources::new();
    visit(project, 0, &mut resources);
    resources
}

fn visit(dir: &Path, depth: usize, out: &mut Resources) {
    if depth > MAX_DEPTH {
        return;
    }
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let in_source_set = path.parent().and_then(Path::parent).and_then(Path::file_name).is_some_and(|name| name == "src");
        if name == "res" && in_source_set {
            index_res(&path, out);
        } else {
            visit(&path, depth + 1, out);
        }
    }
}

fn files(dir: &Path) -> impl Iterator<Item = (String, PathBuf)> {
    std::fs::read_dir(dir).into_iter().flatten().flatten().map(|entry| (entry.file_name().to_string_lossy().into_owned(), entry.path()))
}

fn index_res(res: &Path, out: &mut Resources) {
    for (folder, path) in files(res).filter(|(_, path)| path.is_dir()) {
        let kind = folder.split('-').next().unwrap_or("").to_string();
        for (file, path) in files(&path) {
            let xml = file.ends_with(".xml");
            let content = || std::fs::read_to_string(&path).map(|content| strip_comments(&content)).unwrap_or_default();
            if kind == "values" {
                if xml {
                    values(&content(), out);
                }
                continue;
            }
            let stem = file.split('.').next().unwrap_or("").to_string();
            out.entry(kind.clone()).or_default().insert(stem);
            if xml {
                let content = content();
                for (at, _) in content.match_indices("@+id/") {
                    let name: String = content[at + 5..].chars().take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '.').collect();
                    out.entry("id".into()).or_default().insert(name);
                }
            }
        }
    }
}

fn strip_comments(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find("-->").map_or(rest.len(), |at| start + at + 3);
        out.extend(rest[start..end].chars().map(|ch| if ch == '\n' { '\n' } else { ' ' }));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn attribute<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}=\"");
    let (at, _) = tag.match_indices(&needle).find(|(at, _)| tag[..*at].ends_with(char::is_whitespace))?;
    let value = &tag[at + needle.len()..];
    value.find('"').map(|end| &value[..end])
}

/// Le risorse dichiarate in un file di `values`: `<string name=…>`, `<style>`, `<item type=… name=…>`…
fn values(content: &str, out: &mut Resources) {
    for tag in content.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or("");
        let element = tag.split(|ch: char| ch.is_whitespace() || ch == '/').next().unwrap_or("");
        let kind = match element {
            "string-array" | "integer-array" | "array" => "array",
            "declare-styleable" => "styleable",
            "item" => match attribute(tag, "type") {
                Some(kind) => kind,
                None => continue,
            },
            "" | "resources" | "eat-comment" | "skip" => continue,
            other if other.starts_with(['?', '!', '/']) => continue,
            other => other,
        };
        if let Some(name) = attribute(tag, "name").filter(|name| !name.contains(':')) {
            out.entry(kind.to_string()).or_default().insert(name.to_string());
        }
    }
}

/// Tipo, nome e posizione di ogni riferimento a una risorsa del progetto.
fn references(text: &str, xml: bool) -> Vec<(String, String, Range<usize>)> {
    let name_end = |bytes: &[u8], from: usize, dots: bool| from + bytes[from..].iter().take_while(|byte| is_ident(**byte) || (dots && **byte == b'.')).count();
    let mut list = Vec::new();
    if xml {
        let clean = strip_comments(text);
        let bytes = clean.as_bytes();
        for (at, _) in clean.match_indices('@') {
            // `?attr/…` sono attributi del tema, `@+id/` dichiara, `@android:` e `@pacchetto:` sono di altri.
            let kind_end = at + 1 + bytes[at + 1..].iter().take_while(|byte| byte.is_ascii_lowercase()).count();
            if bytes.get(kind_end) != Some(&b'/') || kind_end == at + 1 {
                continue;
            }
            let end = name_end(bytes, kind_end + 1, true);
            list.push((clean[at + 1..kind_end].to_string(), clean[kind_end + 1..end].to_string(), at..end));
        }
        return list;
    }
    let m = mask(text);
    let mut from = 0;
    while let Some(at) = m[from..].windows(2).position(|pair| pair == b"R.").map(|at| from + at) {
        from = at + 2;
        if (at > 0 && is_ident(m[at - 1])) || m[..at].ends_with(b"android.") {
            continue;
        }
        let kind_end = name_end(&m, at + 2, false);
        if m.get(kind_end) != Some(&b'.') {
            continue;
        }
        let end = name_end(&m, kind_end + 1, false);
        if end > kind_end + 1 {
            list.push((text[at + 2..kind_end].to_string(), text[kind_end + 1..end].to_string(), at..end));
        }
    }
    list
}

/// Riferimenti a risorse che non ci sono in nessun modulo: avvisi, perché possono venire da una libreria.
pub fn check(text: &str, xml: bool, resources: &Resources) -> Vec<Diagnostic> {
    if resources.is_empty() {
        return Vec::new();
    }
    references(text, xml)
        .into_iter()
        .filter(|(kind, name, _)| CHECKED.contains(&kind.as_str()) && !resources.get(kind).is_some_and(|names| names.iter().any(|known| r_name(known) == r_name(name))))
        .map(|(_, _, range)| Diagnostic {
            start: position_of(text, range.start),
            end: position_of(text, range.end),
            message: format!("{} non c'è nelle risorse del progetto", &text[range]),
            error: false,
        })
        .collect()
}

/// `R.` (i tipi), `R.<tipo>.` (i nomi) in Kotlin e Java; `@<tipo>/` (i nomi) negli XML.
pub fn completions(text: &str, caret: usize, xml: bool, resources: impl FnOnce() -> Resources) -> Option<Local> {
    let before = text.get(..caret)?;
    let bytes = before.as_bytes();
    let word_start = caret - bytes.iter().rev().take_while(|byte| is_ident(**byte) || (xml && **byte == b'.')).count();
    let separator = word_start.checked_sub(1)?;
    let ident_before = |end: usize| end - bytes[..end].iter().rev().take_while(|byte| is_ident(**byte)).count();
    if xml {
        if bytes[separator] != b'/' {
            return None;
        }
        let kind_start = ident_before(separator);
        if kind_start == 0 || bytes[kind_start - 1] != b'@' || kind_start == separator {
            return None;
        }
        let names = resources().remove(&before[kind_start..separator])?;
        return Some(Local { start: word_start, labels: names.into_iter().collect(), merge: false });
    }
    let m = mask(before);
    let is_r = |start: usize, end: usize| &before[start..end] == "R" && (start == 0 || !is_ident(m[start - 1])) && !m[..start].ends_with(b"android.");
    if m[separator] != b'.' {
        return None;
    }
    let previous = ident_before(separator);
    if is_r(previous, separator) {
        return Some(Local { start: word_start, labels: resources().into_keys().collect(), merge: false });
    }
    let dot = previous.checked_sub(1).filter(|&dot| m[dot] == b'.' && previous < separator)?;
    if !is_r(ident_before(dot), dot) {
        return None;
    }
    let names = resources().remove(&before[previous..separator])?;
    Some(Local { start: word_start, labels: names.iter().map(|name| r_name(name)).collect(), merge: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Resources {
        let mut resources = Resources::new();
        values(&strip_comments("<resources>\n<!-- <string name=\"old\">x</string> -->\n<string name=\"app_name\" translatable=\"false\">A</string>\n<style name=\"Theme.App\" parent=\"x\"><item name=\"android:x\">1</item></style>\n<item type=\"id\" name=\"spare\"/>\n<string-array name=\"days\"><item>a</item></string-array>\n</resources>"), &mut resources);
        resources.entry("layout".into()).or_default().insert("main".into());
        resources
    }

    #[test]
    fn reads_values() {
        let resources = sample();
        assert!(resources["string"].contains("app_name") && !resources["string"].contains("old"));
        assert!(resources["style"].contains("Theme.App") && resources["id"].contains("spare") && resources["array"].contains("days"));
        assert!(!resources.contains_key("item"));
    }

    #[test]
    fn checks_code_and_xml_references() {
        let resources = sample();
        let code = "val a = R.string.app_name + R.string.missing\nandroid.R.string.ok\n// R.string.commented\nsetContentView(R.layout.main); R.style.Theme_App; R.attr.colorPrimary";
        let messages: Vec<String> = check(code, false, &resources).into_iter().map(|diagnostic| diagnostic.message).collect();
        assert_eq!(messages, vec!["R.string.missing non c'è nelle risorse del progetto"]);
        let xml = "<TextView android:text=\"@string/app_name\" android:id=\"@+id/title\" android:hint=\"@string/nope\" style=\"@style/Other\" android:textColor=\"?attr/x\" android:src=\"@android:drawable/x\"/>";
        let messages: Vec<String> = check(xml, true, &resources).into_iter().map(|diagnostic| diagnostic.message).collect();
        assert_eq!(messages, vec!["@string/nope non c'è nelle risorse del progetto"]);
    }

    #[test]
    fn completes_resource_names() {
        let complete = |text: &str, xml: bool| {
            let caret = text.find('|').unwrap();
            completions(&text.replace('|', ""), caret, xml, sample).map(|local| (local.start, local.labels))
        };
        assert_eq!(complete("x = R.style.Th|", false), Some((12, vec!["Theme_App".to_string()])));
        assert!(complete("x = R.|", false).is_some_and(|(_, kinds)| kinds.contains(&"string".to_string())));
        assert_eq!(complete("text=\"@style/The|", true), Some((13, vec!["Theme.App".to_string()])));
        assert!(complete("android.R.string.|", false).is_none());
        assert!(complete("foo.bar|", false).is_none());
        assert!(complete("text=\"@+id/|", true).is_none());
    }
}

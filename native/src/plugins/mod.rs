//! Plugin dei linguaggi: completamento, type-check e documentazione inline da un server esterno (tsserver per
//! JavaScript/TypeScript, jdtls per Java). Ogni plugin e ogni opzione si accendono dalla pagina impostazioni e
//! valgono subito; il server parte al primo file che lo richiede e si chiude quando non serve più. COBOL non ha server:
//! controllo e indentazione sono in `cobol`.
mod cobol;
mod doc_popup;
mod inline_html;
mod navigation;
mod process;
mod protocol;
mod script_names;
mod servers;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use steroitor_core::git::runner::CommandLog;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, PostMessageW, SetTimer};

use crate::lang::Role;
use crate::sci::*;
use crate::session::Settings;
use crate::theme::Palette;
use doc_popup::DocPopup;
use process::{Incoming, Process, Queue};
use protocol::{Diagnostic, Item, Origin, Protocol, Reply, Server};

pub use process::WM_PLUGIN_EVENT;

pub const COMPLETION_TIMER: usize = 20;
pub const CHECK_TIMER: usize = 21;
const COMPLETION_DELAY_MS: u32 = 120;
const CHECK_DELAY_MS: u32 = 600;
const DWELL_MS: usize = 600;
const ERROR_INDICATOR: usize = 9;
const WARNING_INDICATOR: usize = 10;
const MAX_ITEMS: usize = 300;
const MAX_TIP_LINES: usize = 25;
const MAX_TIP_CHARS: usize = 1500;

pub struct Plugin {
    pub id: &'static str,
    pub name: &'static str,
    /// Cosa serve per farlo funzionare, mostrato nella pagina impostazioni.
    pub requires: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    /// Nome del linguaggio ed estensioni dei suoi file.
    pub languages: &'static [(&'static str, &'static [&'static str])],
    /// Id dell'opzione in `pluginOptions`, etichetta e descrizione, nell'ordine della pagina impostazioni.
    pub options: &'static [(&'static str, &'static str, &'static str)],
    /// None: il plugin lavora senza server.
    protocol: Option<Protocol>,
    language_id: &'static str,
}

pub static PLUGINS: [Plugin; 4] = [
    Plugin {
        id: "javascript",
        name: "JavaScript / TypeScript",
        requires: "tsserver con Node.js, dal progetto, da npm globale o da VS Code",
        version: "1.0.0",
        description: "Completamento, errori di tipo, documentazione e Vai alla dichiarazione con tsserver; HTML inline di React e Angular colorato.",
        languages: &[("JavaScript", &["js", "mjs", "cjs"]), ("JavaScript JSX", &["jsx"]), ("TypeScript", &["ts", "mts", "cts"]), ("TypeScript JSX", &["tsx"])],
        options: &SCRIPT_OPTIONS,
        protocol: Some(Protocol::TsServer),
        language_id: "typescript",
    },
    Plugin {
        id: "java",
        name: "Java",
        requires: "jdtls dall'estensione Java di VS Code o da JDTLS_HOME; 0,3–1 GB di RAM",
        version: "1.1.0",
        description: "Completamento, errori, documentazione e Vai alla dichiarazione con jdtls. Disattivato, resta l'evidenziazione.",
        languages: &[("Java", &["java"])],
        options: &SERVER_OPTIONS,
        protocol: Some(Protocol::Lsp),
        language_id: "java",
    },
    Plugin {
        id: "c",
        name: "C / C++",
        requires: "clangd da LLVM, da Visual Studio (componente Clang), dall'estensione clangd di VS Code, dal PATH o da CLANGD_PATH; per i progetti con più file un compile_commands.json o un compile_flags.txt",
        version: "1.0.0",
        description: "Completamento, errori, documentazione e Vai alla dichiarazione con clangd. Disattivato, resta l'evidenziazione.",
        languages: &[("C", &["c", "h"]), ("C++", &["cc", "cpp", "cxx", "hpp", "hh", "hxx"])],
        options: &SERVER_OPTIONS,
        protocol: Some(Protocol::Lsp),
        language_id: "cpp",
    },
    Plugin {
        id: "cobol",
        name: "COBOL",
        requires: "nessun programma esterno: il controllo è interno e non è un compilatore",
        version: "1.1.0",
        description: "Controllo del sorgente con i tipi dalle PIC e i copybook accanto al file, indentazione per aree A e B. Formato fisso; libero con >>SOURCE FORMAT FREE.",
        languages: &[("COBOL", &["cbl", "cob", "cpy"])],
        options: &COBOL_OPTIONS,
        protocol: None,
        language_id: "cobol",
    },
];

/// Le opzioni che usano il server del plugin. `typeCheck` è la stessa chiave della versione Tauri.
const SERVER_OPTIONS: [(&str, &str, &str); 3] = [
    ("completion", "Completamento", "Suggerimenti del language server mentre scrivi; Ctrl+Spazio li chiede."),
    ("typeCheck", "Type-check", "Errori e avvisi del language server sottolineati nel file aperto."),
    ("docs", "Documentazione inline", "Firma e documentazione del simbolo sotto il mouse."),
];
/// JavaScript/TypeScript ha in più l'HTML inline, colorato senza server.
const SCRIPT_OPTIONS: [(&str, &str, &str); 5] = [
    SERVER_OPTIONS[0],
    SERVER_OPTIONS[1],
    SERVER_OPTIONS[2],
    (REACT, "HTML inline React (JSX)", "Colora i tag JSX nei file .jsx e .tsx; senza server."),
    (ANGULAR, "HTML inline Angular (template)", "Colora l'HTML nei template inline dei componenti Angular; senza server."),
];
const COBOL_OPTIONS: [(&str, &str, &str); 2] = [
    (TYPE_CHECK, "Controllo del sorgente", "Colonne del formato fisso, DIVISION, IF/EVALUATE/SEARCH/PERFORM chiusi, paragrafi e dati non dichiarati (anche nei copybook), tipi dalle PIC: VALUE, aritmetica, MOVE, SET … TO TRUE."),
    (INDENT, "Indentazione COBOL", "Invio nell'area A o B giusta, Tab alle colonne 8, 12, 16…, END-IF, ELSE e WHEN riallineati mentre li scrivi; sempre spazi."),
];
const INDENT: &str = "indent";
const COMPLETION: &str = SERVER_OPTIONS[0].0;
const TYPE_CHECK: &str = SERVER_OPTIONS[1].0;
const DOCS: &str = SERVER_OPTIONS[2].0;
const REACT: &str = "reactInline";
const ANGULAR: &str = "angularInline";
/// Indicatori che colorano il testo dell'HTML inline, uno per ruolo.
const HTML_INDICATORS: [(usize, inline_html::Role); 13] = [
    (11, inline_html::Role::Tag),
    (12, inline_html::Role::Attribute),
    (13, inline_html::Role::Value),
    (14, inline_html::Role::Punctuation),
    (15, inline_html::Role::Text),
    (16, inline_html::Role::Keyword),
    (17, inline_html::Role::Comment),
    (18, inline_html::Role::AngleBracket),
    (19, inline_html::Role::Event),
    (20, inline_html::Role::Number),
    (FUNCTION_INDICATOR, inline_html::Role::Function),
    (PROPERTY_INDICATOR, inline_html::Role::Property),
    (TYPE_INDICATOR, inline_html::Role::Type),
];
const FUNCTION_INDICATOR: usize = 21;
const PROPERTY_INDICATOR: usize = 22;
const TYPE_INDICATOR: usize = 23;
const INDIC_TEXTFORE: isize = 17;

fn plugin_for(path: &Path) -> Option<usize> {
    let extension = path.extension()?.to_string_lossy().to_lowercase();
    PLUGINS.iter().position(|plugin| plugin.languages.iter().any(|(_, extensions)| extensions.contains(&extension.as_str())))
}

impl Plugin {
    /// Opzione accesa: un'opzione mai toccata è spenta se accende un server, accesa se il plugin non ne ha.
    pub fn option_on(&self, settings: &Settings, option: &str) -> bool {
        settings.plugin_option(self.id, option, self.protocol.is_none())
    }
}

/// Cartella del progetto per il server del plugin: un server per progetto.
fn server_root(id: &str, path: &Path) -> PathBuf {
    match id {
        "java" => servers::java_root(path),
        "c" => servers::c_root(path),
        _ => servers::script_root(path),
    }
}

/// Nome e comando del server del plugin; jdtls tiene il suo workspace in `data_dir`.
fn server_command(id: &str, root: &Path, data_dir: Option<&Path>) -> (&'static str, Result<std::process::Command, String>) {
    match id {
        "java" => ("jdtls", data_dir.ok_or_else(|| "APPDATA non impostata".to_string()).and_then(|dir| servers::jdtls(root, dir))),
        "c" => ("clangd", servers::clangd(root)),
        _ => ("tsserver", servers::tsserver(root)),
    }
}

fn wants_server(settings: &Settings, plugin: usize) -> bool {
    let definition = &PLUGINS[plugin];
    definition.protocol.is_some() && settings.plugin_enabled(definition.id) && SERVER_OPTIONS.iter().any(|(option, _, _)| definition.option_on(settings, option))
}

fn option(settings: &Settings, path: Option<&Path>, option: &str) -> Option<usize> {
    let plugin = plugin_for(path?)?;
    (settings.plugin_enabled(PLUGINS[plugin].id) && PLUGINS[plugin].option_on(settings, option)).then_some(plugin)
}

fn indent_unit(sci: &Sci) -> usize {
    match sci.call(SCI_GETINDENT, 0, 0) {
        0 => sci.call(SCI_GETTABWIDTH, 0, 0),
        unit => unit,
    }
    .max(1) as usize
}

/// Indentazione della riga a sole spazi (COBOL va a colonne), con il cursore all'inizio del testo.
fn indent_with_spaces(sci: &Sci, line: usize, columns: usize) {
    let tabs = sci.call(SCI_GETUSETABS, 0, 0) as usize;
    sci.call(SCI_SETUSETABS, 0, 0);
    sci.call(SCI_SETLINEINDENTATION, line, columns as isize);
    sci.call(SCI_SETUSETABS, tabs, 0);
}

/// Colonna della riga nuova dopo Invio quando la decide il plugin (COBOL); None per l'indentazione normale.
pub fn new_line_column(sci: &Sci, settings: &Settings, path: Option<&Path>, line: usize, unit: usize) -> Option<usize> {
    option(settings, path, INDENT)?;
    Some(cobol::new_line_column(&sci.text(), line, unit))
}

/// Tab e Shift+Tab nei file COBOL a formato fisso: alle colonne 8, 12, 16… false se li gestisce Scintilla
/// (formato libero, più cursori, selezione).
pub fn tab(sci: &Sci, settings: &Settings, path: Option<&Path>, back: bool) -> bool {
    let caret = sci.call(SCI_GETCURRENTPOS, 0, 0) as usize;
    if option(settings, path, INDENT).is_none() || sci.call(SCI_GETSELECTIONS, 0, 0) > 1 || sci.call(SCI_GETANCHOR, 0, 0) as usize != caret {
        return false;
    }
    let line = sci.call(SCI_LINEFROMPOSITION, caret, 0).max(0) as usize;
    let column = sci.call(SCI_GETCOLUMN, caret, 0).max(0) as usize;
    let Some(stop) = cobol::tab_stop(&sci.text(), column, false) else { return false };
    if !back {
        sci.call_str(SCI_REPLACESEL, 0, &" ".repeat(stop - column));
        return true;
    }
    // Shift+Tab sposta la riga solo con il cursore nell'indentazione.
    if caret > sci.call(SCI_GETLINEINDENTPOSITION, line, 0) as usize {
        return false;
    }
    let indentation = sci.call(SCI_GETLINEINDENTATION, line, 0).max(0) as usize;
    let target = cobol::tab_stop("", indentation, true).unwrap_or(0);
    sci.call(SCI_BEGINUNDOACTION, 0, 0);
    indent_with_spaces(sci, line, target);
    sci.call(SCI_GOTOPOS, sci.call(SCI_GETLINEINDENTPOSITION, line, 0) as usize, 0);
    sci.call(SCI_ENDUNDOACTION, 0, 0);
    true
}

/// END-IF, ELSE, WHEN… appena scritti da soli sulla riga: la riga va alla colonna del loro IF, EVALUATE…
fn realign_closer(sci: &Sci) {
    let caret = sci.call(SCI_GETCURRENTPOS, 0, 0) as usize;
    let line = sci.call(SCI_LINEFROMPOSITION, caret, 0).max(0) as usize;
    let start = sci.call(SCI_POSITIONFROMLINE, line, 0) as usize;
    let text = sci.line(line);
    let Some((before, after)) = text.split_at_checked(caret - start) else { return };
    // Con numeri di sequenza o altro testo sulla riga, `word` non è una parola sola e closer_column non risponde.
    let word = before.trim().to_ascii_uppercase();
    if !after.trim().is_empty() {
        return;
    }
    let Some(column) = cobol::closer_column(&sci.text(), line, &word, indent_unit(sci)) else { return };
    if sci.call(SCI_GETLINEINDENTATION, line, 0) as usize == column {
        return;
    }
    sci.call(SCI_BEGINUNDOACTION, 0, 0);
    indent_with_spaces(sci, line, column);
    sci.call(SCI_GOTOPOS, sci.call(SCI_GETLINEINDENTPOSITION, line, 0) as usize + word.len(), 0);
    sci.call(SCI_ENDUNDOACTION, 0, 0);
}

pub use navigation::Jump;

/// Ctrl+click e Ctrl+B senza server: nei file del plugin JavaScript/TypeScript e negli HTML, con il plugin attivo.
fn navigable<'a>(settings: &Settings, path: Option<&'a Path>) -> Option<&'a Path> {
    let path = path?;
    let extension = path.extension()?.to_string_lossy().to_lowercase();
    let script = PLUGINS[0].languages.iter().any(|(_, extensions)| extensions.contains(&extension.as_str()));
    (settings.plugin_enabled(PLUGINS[0].id) && (script || extension == "html" || extension == "htm")).then_some(path)
}

/// Il simbolo da sottolineare sotto il mouse con Ctrl premuto.
pub fn link_at(settings: &Settings, path: Option<&Path>, text: &str, position: usize) -> Option<std::ops::Range<usize>> {
    navigable(settings, path)?;
    navigation::target_at(text, position).map(|(range, _)| range)
}

/// Dove porta il simbolo in `position`; None se non si trova (si prova con il server o nel file).
pub fn jump(settings: &Settings, path: Option<&Path>, text: &str, position: usize) -> Option<Jump> {
    let path = navigable(settings, path)?;
    navigation::jump(path, text, position, &servers::script_root(path))
}

/// (riga, colonna UTF-16) dell'offset in byte, come le vogliono i server.
pub fn position_of(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = text.get(..offset).unwrap_or(text);
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    (before.matches('\n').count(), before[line_start..].encode_utf16().count())
}

/// Offset in byte di (riga, colonna UTF-16), fermandosi a fine riga.
pub fn offset_of(text: &str, (line, column): (usize, usize)) -> usize {
    let mut start = 0;
    for _ in 0..line {
        match text[start..].find('\n') {
            Some(index) => start += index + 1,
            None => return text.len(),
        }
    }
    let end = text[start..].find('\n').map_or(text.len(), |index| start + index);
    let mut units = 0;
    for (index, ch) in text[start..end].char_indices() {
        if units >= column {
            return start + index;
        }
        units += ch.len_utf16();
    }
    end
}

fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// Parole del documento che iniziano con il prefisso: il completamento quando il server non c'è o non risponde.
fn document_words(text: &str, prefix: &str) -> Vec<String> {
    let lower = prefix.to_lowercase();
    let mut seen = HashSet::new();
    let mut words: Vec<String> = text
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '$'))
        .filter(|word| word.len() > prefix.len() && !word.starts_with(|ch: char| ch.is_ascii_digit()) && word.to_lowercase().starts_with(&lower))
        .filter(|word| seen.insert(*word))
        .map(str::to_string)
        .collect();
    words.sort_by_key(|word| word.to_lowercase());
    words
}

const DECLARATIONS: [&str; 17] = ["function", "class", "interface", "type", "enum", "const", "let", "var", "def", "fn", "struct", "trait", "record", "namespace", "module", "mod", "macro_rules!"];

/// Offset della dichiarazione di `word` nel testo, senza server: la prima occorrenza preceduta da una parola come
/// `function`, `class`, `const`, `def`, `fn`; altrimenti la prima occorrenza del nome.
pub fn local_declaration(text: &str, word: &str) -> Option<usize> {
    let is_word = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    let occurrences: Vec<usize> = text
        .match_indices(word)
        .map(|(index, _)| index)
        .filter(|&index| !text[..index].ends_with(is_word) && !text[index + word.len()..].starts_with(is_word))
        .collect();
    let declared = occurrences.iter().copied().find(|&index| {
        let before = text[..index].trim_end();
        let previous = &before[before.rfind(|ch: char| !(is_word(ch) || ch == '!')).map_or(0, |at| at + 1)..];
        DECLARATIONS.contains(&previous)
    });
    declared.or_else(|| occurrences.first().copied())
}

fn trim_tip(text: &str) -> String {
    let mut tip: String = text.lines().take(MAX_TIP_LINES).collect::<Vec<_>>().join("\n");
    if tip.len() > MAX_TIP_CHARS {
        let cut = (0..=MAX_TIP_CHARS).rev().find(|index| tip.is_char_boundary(*index)).unwrap_or(0);
        tip.truncate(cut);
        tip.push('…');
    }
    tip
}

struct Running {
    key: String,
    plugin: usize,
    server: Server,
}

struct Pending {
    ticket: u64,
    server: u32,
    origin: Origin,
    document: isize,
    start: usize,
    items: Vec<Item>,
    incomplete: bool,
}

pub struct Host {
    hwnd: HWND,
    data_dir: Option<PathBuf>,
    queue: Queue,
    servers: RefCell<HashMap<u32, Running>>,
    next_server: Cell<u32>,
    /// Server che non sono partiti o si sono chiusi: non si riprovano finché non si cambiano le impostazioni.
    failed: RefCell<HashSet<String>>,
    problems: RefCell<Vec<CommandLog>>,
    ticket: Cell<u64>,
    trigger: Cell<Option<char>>,
    completion: RefCell<Option<Pending>>,
    /// Richiesta di documentazione in corso: ticket e posizione del calltip; senza posizione va accanto alla lista.
    tip: Cell<Option<(u64, Option<usize>)>>,
    popup: DocPopup,
    palette: Palette,
    font: Cell<HFONT>,
    diagnostics: RefCell<HashMap<String, [Vec<Diagnostic>; 2]>>,
    definition: Cell<Option<u64>>,
    /// Risposta a Vai alla dichiarazione, per l'app: Some(None) se il server non ha trovato nulla.
    navigation: RefCell<Option<Option<(PathBuf, (usize, usize))>>>,
}

impl Host {
    pub fn new(hwnd: HWND, data_dir: Option<PathBuf>, palette: Palette) -> Host {
        Host {
            hwnd,
            data_dir,
            queue: Arc::default(),
            servers: RefCell::default(),
            next_server: Cell::new(1),
            failed: RefCell::default(),
            problems: RefCell::default(),
            ticket: Cell::new(0),
            trigger: Cell::new(None),
            completion: RefCell::new(None),
            tip: Cell::new(None),
            popup: DocPopup::new(hwnd),
            palette,
            font: Cell::new(std::ptr::null_mut()),
            diagnostics: RefCell::default(),
            definition: Cell::new(None),
            navigation: RefCell::new(None),
        }
    }

    /// Lista dei suggerimenti, riquadri e sottolineature dell'editor: una volta, sono della vista.
    pub fn configure(sci: &Sci, palette: &Palette) {
        sci.call(SCI_AUTOCSETSEPARATOR, b'\n' as usize, 0);
        sci.call(SCI_AUTOCSETORDER, SC_ORDER_CUSTOM, 0);
        sci.call(SCI_AUTOCSETIGNORECASE, 1, 0);
        sci.call(SCI_AUTOCSETMAXHEIGHT, 10, 0);
        sci.call(SCI_SETMOUSEDWELLTIME, DWELL_MS, 0);
        sci.call(SCI_CALLTIPSETBACK, palette.panel as usize, 0);
        sci.call(SCI_CALLTIPSETFORE, palette.fg as usize, 0);
        let opaque = |color: u32| (color | 0xFF00_0000) as isize;
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_LIST, opaque(palette.fg));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_LIST_BACK, opaque(palette.panel));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_LIST_SELECTED, opaque(palette.fg));
        sci.call(SCI_SETELEMENTCOLOUR, SC_ELEMENT_LIST_SELECTED_BACK, opaque(palette.selection));
        for (indicator, role) in HTML_INDICATORS {
            let color = match role {
                inline_html::Role::Tag | inline_html::Role::AngleBracket => palette.tag,
                inline_html::Role::Attribute => palette.attribute,
                inline_html::Role::Event | inline_html::Role::Property => palette.property,
                inline_html::Role::Value => palette.string,
                inline_html::Role::Punctuation | inline_html::Role::Text => palette.fg,
                inline_html::Role::Keyword => palette.keyword,
                inline_html::Role::Number => palette.number,
                inline_html::Role::Function => palette.function,
                inline_html::Role::Comment => palette.comment,
                inline_html::Role::Type => palette.kind,
            };
            sci.call(SCI_INDICSETSTYLE, indicator, INDIC_TEXTFORE);
            sci.call(SCI_INDICSETFORE, indicator, color as isize);
        }
        for (indicator, color) in [(ERROR_INDICATOR, palette.danger), (WARNING_INDICATOR, palette.number)] {
            sci.call(SCI_INDICSETSTYLE, indicator, INDIC_SQUIGGLEPIXMAP);
            sci.call(SCI_INDICSETFORE, indicator, color as isize);
        }
    }

    /// Font dell'interfaccia, per il riquadro della documentazione.
    pub fn set_font(&self, font: HFONT) {
        self.font.set(font);
    }

    fn show_doc(&self, text: &str) {
        self.popup.show(&trim_tip(text), self.font.get(), &self.palette);
    }

    fn post(&self) {
        // SAFETY: messaggio alla propria finestra.
        unsafe { PostMessageW(self.hwnd, WM_PLUGIN_EVENT, 0, 0) };
    }

    fn problem(&self, command: String, cwd: &Path, message: String) {
        self.problems.borrow_mut().push(CommandLog { id: 0, command, cwd: cwd.display().to_string(), code: None, duration_ms: 0, started_at: 0, stdout: String::new(), stderr: message, background: false });
        self.post();
    }

    /// Il server del plugin per il progetto del file: lo avvia se manca. None se non parte.
    fn server_for(&self, plugin: usize, path: &Path) -> Option<u32> {
        let definition = &PLUGINS[plugin];
        let protocol = definition.protocol?;
        let root = server_root(definition.id, path);
        let server_key = format!("{}:{}", definition.id, key(&root));
        if let Some((id, _)) = self.servers.borrow().iter().find(|(_, running)| running.key == server_key) {
            return Some(*id);
        }
        if self.failed.borrow().contains(&server_key) {
            return None;
        }
        let (name, command) = server_command(definition.id, &root, self.data_dir.as_deref());
        let id = self.next_server.get();
        let process = command.and_then(|command| Process::spawn(command, id, Arc::clone(&self.queue), self.hwnd));
        match process {
            Ok(process) => {
                self.next_server.set(id + 1);
                let server = Server::new(protocol, process, &root, definition.language_id);
                self.servers.borrow_mut().insert(id, Running { key: server_key, plugin, server });
                Some(id)
            }
            Err(error) => {
                self.failed.borrow_mut().insert(server_key);
                self.problem(format!("{name} ({})", definition.name), &root, format!("{name} non è partito: {error}"));
                None
            }
        }
    }

    fn with_server<T>(&self, id: u32, action: impl FnOnce(&mut Server) -> T) -> Option<T> {
        self.servers.borrow_mut().get_mut(&id).map(|running| action(&mut running.server))
    }

    /// Carattere digitato: programma il completamento (subito dopo il punto, altrimenti dopo una breve pausa).
    pub fn on_char(&self, sci: &Sci, settings: &Settings, path: Option<&Path>, ch: i32) {
        if option(settings, path, INDENT).is_some() {
            realign_closer(sci);
        }
        if option(settings, path, COMPLETION).is_none() {
            return;
        }
        let Some(ch) = u32::try_from(ch).ok().and_then(char::from_u32) else { return };
        let word = ch.is_alphanumeric() || ch == '_' || ch == '$';
        // Con la lista aperta filtra Scintilla, a meno che il server non l'abbia tagliata.
        let incomplete = self.completion.borrow().as_ref().is_some_and(|pending| pending.incomplete);
        if ch == '.' {
            sci.call(SCI_AUTOCCANCEL, 0, 0);
        } else if !word || (sci.call(SCI_AUTOCACTIVE, 0, 0) != 0 && !incomplete) {
            return;
        }
        self.trigger.set((ch == '.').then_some('.'));
        // SAFETY: timer della finestra principale; se esiste già viene riavviato.
        unsafe { SetTimer(self.hwnd, COMPLETION_TIMER, if ch == '.' { 1 } else { COMPLETION_DELAY_MS }, None) };
    }

    /// Ctrl+Spazio (`explicit`) o il timer dopo la digitazione.
    pub fn complete(&self, sci: &Sci, settings: &Settings, path: Option<&Path>, explicit: bool) {
        // SAFETY: timer della finestra principale.
        unsafe { KillTimer(self.hwnd, COMPLETION_TIMER) };
        let trigger = self.trigger.take();
        let Some(plugin) = option(settings, path, COMPLETION) else { return };
        let Some(path) = path else { return };
        let caret = sci.call(SCI_GETCURRENTPOS, 0, 0) as usize;
        let start = sci.call(SCI_WORDSTARTPOSITION, caret, 1) as usize;
        let text = sci.text();
        let prefix = text.get(start..caret).unwrap_or("");
        let after_dot = start > 0 && text.as_bytes().get(start - 1) == Some(&b'.');
        if (!explicit && prefix.is_empty() && !after_dot) || prefix.starts_with(|ch: char| ch.is_ascii_digit()) {
            return;
        }
        let Some(server) = self.server_for(plugin, path) else {
            show_list(sci, start, document_words(&text, prefix));
            return;
        };
        let ticket = self.ticket.get() + 1;
        self.ticket.set(ticket);
        let origin = Origin { path: path.to_path_buf(), position: position_of(&text, caret) };
        self.with_server(server, |server| {
            server.sync(path, &text);
            server.completion(ticket, &origin, trigger);
        });
        *self.completion.borrow_mut() = Some(Pending { ticket, server, origin, document: sci.call(SCI_GETDOCPOINTER, 0, 0), start, items: Vec::new(), incomplete: false });
    }

    /// Voce selezionata nella lista: firma e documentazione accanto, se l'opzione è attiva.
    pub fn on_selection(&self, settings: &Settings, path: Option<&Path>, selected: &str) {
        if option(settings, path, DOCS).is_none() {
            return;
        }
        let completion = self.completion.borrow();
        let Some(pending) = completion.as_ref() else { return };
        let Some(item) = pending.items.iter().find(|item| item.insert == selected) else {
            return self.popup.hide();
        };
        if let Some(doc) = &item.doc {
            self.tip.set(None);
            return self.show_doc(doc);
        }
        let ticket = self.ticket.get() + 1;
        self.ticket.set(ticket);
        self.tip.set(Some((ticket, None)));
        let (server, origin, item) = (pending.server, pending.origin.clone(), item.clone());
        drop(completion);
        self.with_server(server, |server| server.item_doc(ticket, &origin, &item));
    }

    /// Vai alla dichiarazione dal server del plugin; false se il plugin o il server non c'è (l'app cerca nel file).
    pub fn definition(&self, sci: &Sci, settings: &Settings, path: Option<&Path>) -> bool {
        let Some((plugin, path)) = path.and_then(|path| plugin_for(path).map(|plugin| (plugin, path))) else { return false };
        if !wants_server(settings, plugin) {
            return false;
        }
        let Some(server) = self.server_for(plugin, path) else { return false };
        let text = sci.text();
        let caret = sci.call(SCI_GETCURRENTPOS, 0, 0) as usize;
        let ticket = self.ticket.get() + 1;
        self.ticket.set(ticket);
        self.definition.set(Some(ticket));
        let origin = Origin { path: path.to_path_buf(), position: position_of(&text, caret) };
        self.with_server(server, |server| {
            server.sync(path, &text);
            server.definition(ticket, &origin);
        });
        true
    }

    pub fn take_navigation(&self) -> Option<Option<(PathBuf, (usize, usize))>> {
        self.navigation.borrow_mut().take()
    }

    pub fn on_list_closed(&self, sci: &Sci) {
        self.tip.set(None);
        self.popup.hide();
        sci.call(SCI_CALLTIPCANCEL, 0, 0);
    }

    /// Mouse fermo sul testo: il problema in quel punto, altrimenti la documentazione del simbolo.
    pub fn on_dwell(&self, sci: &Sci, settings: &Settings, path: Option<&Path>, position: isize) {
        if position < 0 || sci.call(SCI_AUTOCACTIVE, 0, 0) != 0 {
            return;
        }
        let position = position as usize;
        let text = sci.text();
        let point = position_of(&text, position);
        if option(settings, path, TYPE_CHECK).is_some() {
            let found = path.and_then(|path| {
                let diagnostics = self.diagnostics.borrow();
                let slots = diagnostics.get(&key(path))?;
                slots.iter().flatten().find(|diagnostic| diagnostic.start <= point && (point < diagnostic.end || diagnostic.start == diagnostic.end && point == diagnostic.start)).map(|diagnostic| diagnostic.message.clone())
            });
            if let Some(message) = found {
                return show_tip(sci, position, &message);
            }
        }
        let (Some(plugin), Some(path)) = (option(settings, path, DOCS), path) else { return };
        let Some(server) = self.server_for(plugin, path) else { return };
        let ticket = self.ticket.get() + 1;
        self.ticket.set(ticket);
        self.tip.set(Some((ticket, Some(position))));
        let origin = Origin { path: path.to_path_buf(), position: point };
        self.with_server(server, |server| {
            server.sync(path, &text);
            server.hover(ticket, &origin);
        });
    }

    pub fn on_dwell_end(&self, sci: &Sci) {
        if sci.call(SCI_AUTOCACTIVE, 0, 0) == 0 {
            self.on_list_closed(sci);
        }
    }

    pub fn on_modified(&self, settings: &Settings, path: Option<&Path>) {
        if option(settings, path, TYPE_CHECK).is_some() {
            // SAFETY: timer della finestra principale; se esiste già viene riavviato.
            unsafe { SetTimer(self.hwnd, CHECK_TIMER, CHECK_DELAY_MS, None) };
        }
    }

    /// Manda il testo al server e chiede il controllo; la diagnostica arriva dopo, con WM_PLUGIN_EVENT.
    pub fn check(&self, sci: &Sci, settings: &Settings, path: Option<&Path>) {
        // SAFETY: timer della finestra principale.
        unsafe { KillTimer(self.hwnd, CHECK_TIMER) };
        let (Some(plugin), Some(path)) = (option(settings, path, TYPE_CHECK), path) else { return };
        if PLUGINS[plugin].protocol.is_none() {
            self.diagnostics.borrow_mut().insert(key(path), [cobol::check(&sci.text(), path.parent()), Vec::new()]);
            return self.apply_diagnostics(sci, settings, Some(path));
        }
        let Some(server) = self.server_for(plugin, path) else { return };
        let text = sci.text();
        self.with_server(server, |server| {
            server.sync(path, &text);
            server.check(path);
        });
    }

    /// Nei file JavaScript/TypeScript colora funzioni, tipi e proprietà come la versione Tauri (sempre) e, secondo le
    /// opzioni del plugin, il JSX e i template Angular come HTML, gli `styles` Angular come CSS.
    pub fn highlight(&self, sci: &Sci, settings: &Settings, path: Option<&Path>) {
        let length = sci.call(SCI_GETLENGTH, 0, 0);
        for indicator in HTML_INDICATORS.iter().map(|(indicator, _)| *indicator) {
            sci.call(SCI_SETINDICATORCURRENT, indicator, 0);
            sci.call(SCI_INDICATORCLEARRANGE, 0, length);
        }
        let extension = path.and_then(|path| path.extension()).map(|extension| extension.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !PLUGINS[0].languages.iter().any(|(_, extensions)| extensions.contains(&extension.as_str())) {
            return;
        }
        let fill = |indicator: usize, range: &std::ops::Range<usize>| {
            sci.call(SCI_SETINDICATORCURRENT, indicator, 0);
            sci.call(SCI_INDICATORFILLRANGE, range.start, range.len() as isize);
        };
        let text = sci.text();
        let react = ["jsx", "tsx", "js", "mjs", "cjs"].contains(&extension.as_str()) && option(settings, path, REACT).is_some();
        let angular = option(settings, path, ANGULAR).is_some();
        let html = if react || angular { inline_html::spans(&text, react, angular) } else { Vec::new() };
        for (range, role) in &html {
            let Some(&(indicator, _)) = HTML_INDICATORS.iter().find(|(_, candidate)| candidate == role) else { continue };
            fill(indicator, range);
        }
        let taken = merged(html.into_iter().map(|(range, _)| range).collect());
        for (range, role) in script_names::spans(&text) {
            let at = taken.partition_point(|taken| taken.end <= range.start);
            if taken.get(at).is_some_and(|taken| taken.start < range.end) {
                continue;
            }
            let indicator = match role {
                Role::Function => FUNCTION_INDICATOR,
                Role::Property => PROPERTY_INDICATOR,
                _ => TYPE_INDICATOR,
            };
            fill(indicator, &range);
        }
    }

    /// Tab diventata attiva: le sue sottolineature e un controllo aggiornato.
    pub fn on_activate(&self, sci: &Sci, settings: &Settings, path: Option<&Path>) {
        self.on_list_closed(sci);
        self.highlight(sci, settings, path);
        self.apply_diagnostics(sci, settings, path);
        self.check(sci, settings, path);
    }

    pub fn close(&self, path: &Path) {
        self.diagnostics.borrow_mut().remove(&key(path));
        for running in self.servers.borrow_mut().values_mut() {
            running.server.close(path);
        }
    }

    /// Impostazioni cambiate: si chiudono i server che non servono più e si riprovano quelli falliti.
    /// Sottolineature e controllo si aggiornano con la riattivazione della tab.
    pub fn apply_settings(&self, settings: &Settings) {
        self.failed.borrow_mut().clear();
        self.servers.borrow_mut().retain(|_, running| {
            let keep = wants_server(settings, running.plugin);
            if !keep {
                running.server.kill();
            }
            keep
        });
    }

    pub fn shutdown(&self) {
        for running in self.servers.borrow_mut().values_mut() {
            running.server.kill();
        }
    }

    /// Messaggi arrivati dai server; restituisce i problemi da mostrare nella Console.
    pub fn on_event(&self, sci: &Sci, settings: &Settings, path: Option<&Path>) -> Vec<CommandLog> {
        let incoming = self.queue.lock().map(|mut queue| std::mem::take(&mut *queue)).unwrap_or_default();
        for message in incoming {
            match message {
                Incoming::Message(id, value) => {
                    let Some(reply) = self.with_server(id, |server| server.receive(&value)).flatten() else { continue };
                    self.on_reply(sci, settings, path, reply);
                }
                Incoming::Exited(id, error) => {
                    let Some(running) = self.servers.borrow_mut().remove(&id) else { continue };
                    let definition = &PLUGINS[running.plugin];
                    self.failed.borrow_mut().insert(running.key);
                    let detail = if error.trim().is_empty() { "nessun messaggio di errore".into() } else { error };
                    self.problem(format!("server di {}", definition.name), Path::new(""), format!("Il server si è chiuso; si riprova cambiando le impostazioni dei plugin.\n{detail}"));
                }
            }
        }
        std::mem::take(&mut *self.problems.borrow_mut())
    }

    fn on_reply(&self, sci: &Sci, settings: &Settings, path: Option<&Path>, reply: Reply) {
        match reply {
            Reply::Completions { ticket, mut items, incomplete } => {
                let mut completion = self.completion.borrow_mut();
                let Some(pending) = completion.as_mut().filter(|pending| pending.ticket == ticket) else { return };
                if pending.document != sci.call(SCI_GETDOCPOINTER, 0, 0) {
                    return;
                }
                items.sort_by(|a, b| a.sort.cmp(&b.sort));
                let start = pending.start;
                let mut labels: Vec<String> = items.iter().map(|item| item.insert.clone()).collect();
                pending.items = items;
                pending.incomplete = incomplete;
                drop(completion);
                // Senza risposta utile, le parole del documento; non dopo un punto, dove servono i membri.
                let caret = sci.call(SCI_GETCURRENTPOS, 0, 0) as usize;
                if labels.is_empty() && caret > start {
                    let text = sci.text();
                    labels = document_words(&text, text.get(start..caret).unwrap_or(""));
                }
                show_list(sci, start, labels);
            }
            Reply::Doc(ticket, text) => {
                let Some((_, position)) = self.tip.get().filter(|(expected, _)| *expected == ticket) else { return };
                if text.trim().is_empty() {
                    return;
                }
                match position {
                    Some(position) => show_tip(sci, position, &text),
                    None if sci.call(SCI_AUTOCACTIVE, 0, 0) != 0 => self.show_doc(&text),
                    None => {}
                }
            }
            Reply::Definition(ticket, found) => {
                if self.definition.get() == Some(ticket) {
                    self.definition.set(None);
                    *self.navigation.borrow_mut() = Some(found);
                }
            }
            Reply::Diagnostics { path: file, slot, list } => {
                let file_key = key(&file);
                self.diagnostics.borrow_mut().entry(file_key.clone()).or_default()[slot] = list;
                if path.is_some_and(|path| key(path) == file_key) {
                    self.apply_diagnostics(sci, settings, path);
                }
            }
        }
    }

    fn apply_diagnostics(&self, sci: &Sci, settings: &Settings, path: Option<&Path>) {
        let length = sci.call(SCI_GETLENGTH, 0, 0);
        for indicator in [ERROR_INDICATOR, WARNING_INDICATOR] {
            sci.call(SCI_SETINDICATORCURRENT, indicator, 0);
            sci.call(SCI_INDICATORCLEARRANGE, 0, length);
        }
        let Some(path) = path.filter(|_| option(settings, path, TYPE_CHECK).is_some()) else { return };
        let diagnostics = self.diagnostics.borrow();
        let Some(slots) = diagnostics.get(&key(path)) else { return };
        let text = sci.text();
        for diagnostic in slots.iter().flatten() {
            let start = offset_of(&text, diagnostic.start);
            let end = offset_of(&text, diagnostic.end).max(start + 1).min(text.len());
            if end <= start {
                continue;
            }
            sci.call(SCI_SETINDICATORCURRENT, if diagnostic.error { ERROR_INDICATOR } else { WARNING_INDICATOR }, 0);
            sci.call(SCI_INDICATORFILLRANGE, start, (end - start) as isize);
        }
    }
}

/// Intervalli ordinati e senza sovrapposizioni.
fn merged(mut ranges: Vec<std::ops::Range<usize>>) -> Vec<std::ops::Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    for range in ranges {
        match out.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => out.push(range),
        }
    }
    out
}

/// Mostra i suggerimenti che iniziano con quanto scritto da `start` al cursore.
fn show_list(sci: &Sci, start: usize, labels: Vec<String>) {
    let caret = sci.call(SCI_GETCURRENTPOS, 0, 0) as usize;
    if caret < start {
        return;
    }
    let text = sci.text();
    let prefix = text.get(start..caret).unwrap_or("").to_lowercase();
    let mut seen = HashSet::new();
    let shown: Vec<String> = labels.into_iter().filter(|label| !label.is_empty() && !label.contains('\n') && label.to_lowercase().starts_with(&prefix) && seen.insert(label.clone())).take(MAX_ITEMS).collect();
    if shown.is_empty() || (shown.len() == 1 && shown[0].to_lowercase() == prefix) {
        return;
    }
    sci.call_str(SCI_AUTOCSHOW, caret - start, &shown.join("\n"));
}

fn show_tip(sci: &Sci, position: usize, text: &str) {
    sci.call_str(SCI_CALLTIPSHOW, position, &trim_tip(text));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_offsets_and_utf16_positions() {
        let text = "ab\r\nàé😀x\nlast";
        let x = text.find('x').unwrap();
        assert_eq!(position_of(text, x), (1, 4));
        assert_eq!(offset_of(text, (1, 4)), x);
        assert_eq!(offset_of(text, (1, 99)), text.find("\nlast").unwrap());
        assert_eq!(offset_of(text, (9, 0)), text.len());
        assert_eq!(position_of(text, text.len()), (2, 4));
    }

    #[test]
    fn suggests_document_words_by_prefix() {
        let words = document_words("let counter = 1; Count(counter); cont 9x", "co");
        assert_eq!(words, vec!["cont", "Count", "counter"]);
        assert!(document_words("abc", "abc").is_empty());
    }

    #[test]
    fn finds_the_plugin_by_extension() {
        assert_eq!(plugin_for(Path::new("a/B.TSX")).map(|index| PLUGINS[index].id), Some("javascript"));
        assert_eq!(plugin_for(Path::new("Main.java")).map(|index| PLUGINS[index].id), Some("java"));
        assert_eq!(plugin_for(Path::new("PAYROLL.CBL")).map(|index| PLUGINS[index].id), Some("cobol"));
        assert_eq!(plugin_for(Path::new("src/main.c")).map(|index| PLUGINS[index].id), Some("c"));
        assert_eq!(plugin_for(Path::new("Widget.HPP")).map(|index| PLUGINS[index].id), Some("c"));
        assert!(plugin_for(Path::new("x.rs")).is_none());
    }

    #[test]
    fn options_need_the_plugin_enabled() {
        let mut settings = Settings::default();
        let path = Some(Path::new("a.ts"));
        assert!(option(&settings, path, COMPLETION).is_none());
        settings.plugin_options.entry("javascript".into()).or_default().insert(COMPLETION.into(), true);
        assert!(option(&settings, path, COMPLETION).is_some());
        assert!(wants_server(&settings, 0) && !wants_server(&settings, 1));
        settings.disabled_plugins.push("javascript".into());
        assert!(option(&settings, path, COMPLETION).is_none());
    }

    /// Server vero: completamento, documentazione e diagnostica. `cargo test -- --ignored real_server`.
    fn real_server(plugin: usize, file: &str, source: &str, caret: &str, wanted: &str) {
        let dir = std::env::temp_dir().join(format!("steroitor-real-{}-{}", PLUGINS[plugin].id, std::process::id()));
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        if PLUGINS[plugin].id == "java" {
            let pom ="<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><groupId>x</groupId><artifactId>x</artifactId><version>1</version>\
                <properties><maven.compiler.release>17</maven.compiler.release></properties></project>";
            std::fs::write(dir.join("pom.xml"), pom).unwrap();
        }
        let root = server_root(PLUGINS[plugin].id, &path);
        let command = server_command(PLUGINS[plugin].id, &root, Some(&dir.with_extension("data"))).1.unwrap();
        let queue: Queue = Arc::default();
        let process = Process::spawn(command, 1, Arc::clone(&queue), std::ptr::null_mut()).unwrap();
        let mut server = Server::new(PLUGINS[plugin].protocol.unwrap(), process, &root, PLUGINS[plugin].language_id);
        server.sync(&path, source);
        server.check(&path);
        let origin = Origin { path: path.clone(), position: position_of(source, source.find(caret).unwrap() + caret.len()) };
        server.completion(1, &origin, Some('.'));
        let (mut items, mut diagnostics, mut doc) = (None, None, None);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        while std::time::Instant::now() < deadline && (items.is_none() || diagnostics.is_none() || doc.is_none()) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let incoming = std::mem::take(&mut *queue.lock().unwrap());
            for message in incoming {
                let Incoming::Message(_, value) = message else { panic!("server chiuso") };
                match server.receive(&value) {
                    // jdtls risponde vuoto finché non ha importato il progetto: si richiede.
                    Some(Reply::Completions { items: list, .. }) => match list.iter().find(|item| item.insert == wanted) {
                        Some(item) => {
                            server.item_doc(2, &origin, item);
                            items = Some(list);
                        }
                        None => {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            server.completion(1, &origin, Some('.'));
                        }
                    },
                    Some(Reply::Doc(_, text)) => doc = Some(text),
                    Some(Reply::Diagnostics { list, .. }) if !list.is_empty() => diagnostics = Some(list),
                    _ => {}
                }
            }
        }
        server.kill();
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(dir.with_extension("data"));
        eprintln!("doc: {doc:?}\ndiagnostics: {diagnostics:?}");
        assert!(items.is_some_and(|items| !items.is_empty()));
        assert!(doc.is_some_and(|doc| !doc.is_empty()));
        assert!(diagnostics.is_some_and(|list| list.iter().any(|diagnostic| diagnostic.error)));
    }

    #[test]
    #[ignore]
    fn real_server_tsserver_definition() {
        let dir = std::env::temp_dir().join(format!("steroitor-definition-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.ts");
        let source = "export const label = 'x';\nconsole.log(label);\n";
        std::fs::write(&path, source).unwrap();
        let queue: Queue = Arc::default();
        let process = Process::spawn(servers::tsserver(&dir).unwrap(), 1, Arc::clone(&queue), std::ptr::null_mut()).unwrap();
        let mut server = Server::new(Protocol::TsServer, process, &dir, "typescript");
        server.sync(&path, source);
        server.definition(1, &Origin { path: path.clone(), position: position_of(source, source.rfind("label").unwrap() + 2) });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut found = None;
        while found.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(100));
            for message in std::mem::take(&mut *queue.lock().unwrap()) {
                if let Incoming::Message(_, value) = message {
                    if let Some(Reply::Definition(_, location)) = server.receive(&value) {
                        found = Some(location);
                    }
                }
            }
        }
        server.kill();
        let _ = std::fs::remove_dir_all(&dir);
        let (file, position) = found.flatten().expect("definizione");
        assert_eq!(file.file_name().unwrap(), "a.ts");
        assert_eq!(position, (0, 13));
    }

    #[test]
    #[ignore]
    fn real_server_tsserver() {
        real_server(0, "a.ts", "const label: string = 'x';\nconst n: number = label;\nlabel.", "label.", "trim");
    }

    #[test]
    #[ignore]
    fn real_server_jdtls() {
        real_server(1, "src/main/java/Main.java", "public class Main {\n  void f() {\n    String s = \"x\";\n    int n = s;\n    s.tr\n  }\n}\n", "s.tr", "trim");
    }

    #[test]
    #[ignore]
    fn real_server_clangd() {
        let source = "#include <string>\n/// Somma due interi.\nint add(int a, int b) { return a + b; }\nint main() {\n  int n = \"x\";\n  std::string s;\n  s.si\n}\n";
        real_server(2, "main.cpp", source, "s.si", "size");
    }

    #[test]
    fn finds_local_declarations() {
        let text = "call(total);\nconst total = 1;\nfunction run() {}\nrun();";
        assert_eq!(local_declaration(text, "total"), text.find("total ="));
        assert_eq!(local_declaration(text, "run"), text.find("run()"));
        assert_eq!(local_declaration("x = subtotal; total", "total"), Some(14));
        assert_eq!(local_declaration("nothing", "missing"), None);
    }

    #[test]
    fn trims_long_tips() {
        let long = "x\n".repeat(100);
        assert_eq!(trim_tip(&long).lines().count(), MAX_TIP_LINES);
        assert!(trim_tip(&"è".repeat(2000)).ends_with('…'));
    }
}

//! Linguaggi riconosciuti dall'estensione: lexer di Lexilla, parole chiave e colore di ogni stile.
use crate::theme::Palette;

#[derive(Clone, Copy)]
pub enum Role {
    Keyword,
    String,
    Number,
    Comment,
    Function,
    Type,
    Property,
    Tag,
    Attribute,
    Muted,
    Danger,
}

impl Role {
    pub fn color(self, palette: &Palette) -> u32 {
        match self {
            Role::Keyword => palette.keyword,
            Role::String => palette.string,
            Role::Number => palette.number,
            Role::Comment => palette.comment,
            Role::Function => palette.function,
            Role::Type => palette.kind,
            Role::Property => palette.property,
            Role::Tag => palette.tag,
            Role::Attribute => palette.attribute,
            Role::Muted => palette.muted,
            Role::Danger => palette.danger,
        }
    }

    /// I commenti in corsivo, come nella versione Tauri.
    pub fn italic(self) -> bool {
        matches!(self, Role::Comment)
    }
}

pub struct Language {
    pub name: &'static str,
    /// Nome del lexer nel catalogo di `csrc/lexers.cxx`; None = testo semplice.
    pub lexer: Option<&'static str>,
    pub keywords: &'static [&'static str],
    pub properties: &'static [(&'static str, &'static str)],
    pub styles: &'static [(usize, Role)],
}

use Role::*;

const C_STYLES: &[(usize, Role)] = &[
    (1, Comment), (2, Comment), (3, Comment), (15, Comment), (17, Comment), (23, Comment), (24, Comment),
    (4, Number), (5, Keyword), (6, String), (7, String), (12, String), (13, String), (14, String), (20, String), (21, String),
    (9, Muted), (16, Type), (19, Type), (27, Keyword),
];

const JS_KEYWORDS: &str = "abstract any as async await boolean break case catch class const constructor continue debugger declare default delete do else enum export extends false finally for from function get if implements import in infer instanceof interface is keyof let module namespace never new null number object of override package private protected public readonly return satisfies set static string super switch symbol this throw true try type typeof undefined unique unknown var void while with yield";
const JAVA_KEYWORDS: &str = "abstract assert boolean break byte case catch char class const continue default do double else enum extends false final finally float for goto if implements import instanceof int interface long native new non-sealed null package permits private protected public record return sealed short static strictfp super switch synchronized this throw throws transient true try var void volatile while yield";
const C_KEYWORDS: &str = "alignas alignof auto bool break case catch char char16_t char32_t char8_t class concept const consteval constexpr constinit const_cast continue co_await co_return co_yield decltype default delete do double dynamic_cast else enum explicit export extern false float for friend goto if inline int long mutable namespace new noexcept nullptr operator private protected public register reinterpret_cast requires return short signed sizeof static static_assert static_cast struct switch template this thread_local throw true try typedef typeid typename union unsigned using virtual void volatile wchar_t while";
const CSHARP_KEYWORDS: &str = "abstract as async await base bool break byte case catch char checked class const continue decimal default delegate do double else enum event explicit extern false finally fixed float for foreach get goto if implicit in init int interface internal is lock long namespace new null object operator out override params partial private protected public readonly record ref return sbyte sealed set short sizeof stackalloc static string struct switch this throw true try typeof uint ulong unchecked unsafe ushort using var virtual void volatile when where while with yield";
const PYTHON_KEYWORDS: &str = "False None True and as assert async await break case class continue def del elif else except finally for from global if import in is lambda match nonlocal not or pass raise return try type while with yield";
const RUST_KEYWORDS: &str = "as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type union unsafe use where while";
const RUST_TYPES: &str = "bool char f32 f64 i8 i16 i32 i64 i128 isize str u8 u16 u32 u64 u128 usize String Vec Option Result Box Some None Ok Err";
const SQL_KEYWORDS: &str = "add all alter and as asc begin between by case check column commit constraint create cross database default delete desc distinct drop else end exists foreign from full group having if in index inner insert into is join key left like limit not null on or order outer primary references right rollback select set table then top truncate union unique update values view when where with";
const PS_KEYWORDS: &str = "begin break catch class continue data default do dynamicparam else elseif end enum exit filter finally for foreach from function hidden if in param process return static switch throw trap try until using var while";
const BATCH_KEYWORDS: &str = "call cd chdir cls copy defined del dir do echo else endlocal equ errorlevel exist exit for geq goto gtr if in leq lss md mkdir move neq not nul pause popd pushd rd rem ren rename rmdir set setlocal shift start type";

const fn c_like(name: &'static str, keywords: &'static [&'static str], properties: &'static [(&'static str, &'static str)]) -> Language {
    Language { name, lexer: Some("cpp"), keywords, properties, styles: C_STYLES }
}

const SCRIPT_PROPERTIES: &[(&str, &str)] = &[("lexer.cpp.backquoted.strings", "1"), ("lexer.cpp.allow.dollars", "1")];

pub static PLAIN: Language = Language { name: "Testo semplice", lexer: None, keywords: &[], properties: &[], styles: &[] };
static JAVASCRIPT: Language = c_like("JavaScript", &[JS_KEYWORDS], SCRIPT_PROPERTIES);
static TYPESCRIPT: Language = c_like("TypeScript", &[JS_KEYWORDS], SCRIPT_PROPERTIES);
static JAVA: Language = c_like("Java", &[JAVA_KEYWORDS], &[]);
static C: Language = c_like("C / C++", &[C_KEYWORDS], &[]);
static CSHARP: Language = c_like("C#", &[CSHARP_KEYWORDS], &[("lexer.cpp.verbatim.strings.allow.escapes", "0")]);
static PYTHON: Language = Language {
    name: "Python",
    lexer: Some("python"),
    keywords: &[PYTHON_KEYWORDS],
    properties: &[],
    styles: &[
        (1, Comment), (12, Comment), (2, Number), (3, String), (4, String), (6, String), (7, String), (13, String), (16, String),
        (17, String), (18, String), (19, String), (5, Keyword), (8, Type), (9, Function), (14, Keyword), (15, Attribute),
    ],
};
static JSON: Language = Language {
    name: "JSON",
    lexer: Some("json"),
    keywords: &["false true null"],
    properties: &[("lexer.json.allow.comments", "1")],
    styles: &[(1, Number), (2, String), (3, String), (4, Property), (5, Keyword), (6, Comment), (7, Comment), (11, Keyword), (13, Danger)],
};
static YAML: Language = Language {
    name: "YAML",
    lexer: Some("yaml"),
    keywords: &["true false yes no null on off"],
    properties: &[],
    styles: &[(1, Comment), (2, Property), (3, Keyword), (4, Number), (5, Type), (6, Muted), (8, Danger)],
};
static MARKDOWN: Language = Language {
    name: "Markdown",
    lexer: Some("markdown"),
    keywords: &[],
    properties: &[],
    styles: &[
        (2, Keyword), (3, Keyword), (4, Type), (5, Type), (6, Keyword), (7, Keyword), (8, Keyword), (9, Keyword), (10, Keyword),
        (11, Keyword), (13, Number), (14, Number), (15, Comment), (16, Muted), (17, Muted), (18, String), (19, Function),
        (20, Function), (21, Function), (22, Muted), (23, Muted), (24, Property),
    ],
};
const MARKUP_STYLES: &[(usize, Role)] = &[
    (1, Tag), (2, Tag), (11, Tag), (12, Tag), (13, Tag), (3, Attribute), (4, Attribute), (5, Number), (6, String), (7, String),
    (19, String), (9, Comment), (20, Comment), (10, Number), (17, String), (21, Muted),
    (42, Comment), (43, Comment), (44, Comment), (45, Number), (47, Keyword), (48, String), (49, String), (52, String), (53, String),
];
static HTML: Language = Language { name: "HTML", lexer: Some("hypertext"), keywords: &["", JS_KEYWORDS], properties: &[], styles: MARKUP_STYLES };
static XML: Language = Language { name: "XML", lexer: Some("xml"), keywords: &[], properties: &[], styles: MARKUP_STYLES };
static CSS: Language = Language {
    name: "CSS",
    lexer: Some("css"),
    keywords: &[],
    properties: &[],
    styles: &[
        (1, Tag), (2, Type), (3, Keyword), (4, Keyword), (6, Property), (7, Property), (8, String), (9, Comment), (10, Type),
        (11, Keyword), (12, Keyword), (13, String), (14, String), (15, Property), (16, Attribute), (17, Property), (18, Keyword),
        (19, Property), (22, Keyword), (23, Property),
    ],
};
static RUST: Language = Language {
    name: "Rust",
    lexer: Some("rust"),
    keywords: &[RUST_KEYWORDS, RUST_TYPES],
    properties: &[],
    styles: &[
        (1, Comment), (2, Comment), (3, Comment), (4, Comment), (5, Number), (6, Keyword), (7, Type), (13, String), (14, String),
        (15, String), (18, Type), (19, Function), (20, Danger), (21, String), (22, String), (23, String), (24, String), (25, String),
    ],
};
static TOML: Language = Language {
    name: "TOML",
    lexer: Some("toml"),
    keywords: &["true false"],
    properties: &[],
    styles: &[(1, Comment), (3, Keyword), (4, Number), (5, Tag), (6, Property), (7, Danger), (9, String), (10, String), (11, String), (12, String), (14, Number)],
};
static PROPERTIES: Language = Language {
    name: "Properties / INI",
    lexer: Some("props"),
    keywords: &[],
    properties: &[],
    styles: &[(1, Comment), (2, Tag), (4, Keyword), (5, Property)],
};
static BATCH: Language = Language {
    name: "Batch",
    lexer: Some("batch"),
    keywords: &[BATCH_KEYWORDS],
    properties: &[],
    styles: &[(1, Comment), (2, Keyword), (3, Tag), (4, Muted), (5, Function), (6, Property)],
};
static POWERSHELL: Language = Language {
    name: "PowerShell",
    lexer: Some("powershell"),
    keywords: &[PS_KEYWORDS],
    properties: &[],
    styles: &[(1, Comment), (13, Comment), (16, Comment), (2, String), (3, String), (14, String), (15, String), (4, Number), (5, Property), (8, Keyword), (9, Function), (10, Function), (11, Function)],
};
static SQL: Language = Language {
    name: "SQL",
    lexer: Some("sql"),
    keywords: &[SQL_KEYWORDS],
    properties: &[],
    styles: &[(1, Comment), (2, Comment), (3, Comment), (15, Comment), (4, Number), (5, Keyword), (6, String), (7, String), (16, Type), (23, Property)],
};
static DIFF: Language = Language {
    name: "Diff",
    lexer: Some("diff"),
    keywords: &[],
    properties: &[],
    styles: &[(1, Comment), (2, Keyword), (3, Tag), (4, Type), (5, Danger), (6, String), (7, Function), (8, String), (9, Danger)],
};

const BY_EXTENSION: &[(&[&str], &Language)] = &[
    (&["js", "mjs", "cjs", "jsx"], &JAVASCRIPT),
    (&["ts", "mts", "cts", "tsx"], &TYPESCRIPT),
    (&["java"], &JAVA),
    (&["c", "h", "cc", "cpp", "cxx", "hpp", "hh", "hxx"], &C),
    (&["cs"], &CSHARP),
    (&["py", "pyw"], &PYTHON),
    (&["json", "jsonc"], &JSON),
    (&["yml", "yaml"], &YAML),
    (&["md", "markdown"], &MARKDOWN),
    (&["html", "htm", "svelte", "vue"], &HTML),
    (&["xml", "svg", "xaml", "csproj", "props", "targets"], &XML),
    (&["css", "scss", "less"], &CSS),
    (&["rs"], &RUST),
    (&["toml"], &TOML),
    (&["ini", "cfg", "conf", "properties", "env", "gitconfig", "editorconfig"], &PROPERTIES),
    (&["bat", "cmd"], &BATCH),
    (&["ps1", "psm1", "psd1"], &POWERSHELL),
    (&["sql"], &SQL),
    (&["diff", "patch"], &DIFF),
];

pub fn detect(path: &std::path::Path) -> &'static Language {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("").to_lowercase();
    let extension = name.rsplit('.').next().unwrap_or("");
    BY_EXTENSION.iter().find(|(extensions, _)| extensions.contains(&extension)).map_or(&PLAIN, |(_, language)| language)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn detects_by_extension_ignoring_case() {
        assert_eq!(detect(Path::new("C:\\src\\Main.JAVA")).name, "Java");
        assert_eq!(detect(Path::new("app.component.ts")).name, "TypeScript");
        assert_eq!(detect(Path::new(".gitconfig")).name, "Properties / INI");
        assert_eq!(detect(Path::new("README")).name, "Testo semplice");
    }
}

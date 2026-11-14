//! Compila Scintilla, Lexilla (solo i lexer usati) e il catalogo dei lexer in una libreria statica.
use std::path::Path;

const LIBS: [&str; 8] = ["user32", "gdi32", "imm32", "ole32", "oleaut32", "advapi32", "msimg32", "uuid"];

fn sources(dir: &str) -> impl Iterator<Item = std::path::PathBuf> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "cxx"))
}

fn main() {
    let dirs = ["vendor/scintilla/src", "vendor/scintilla/win32", "vendor/lexilla/lexlib", "vendor/lexilla/lexers"];
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .flag_if_supported("/utf-8")
        .flag_if_supported("/EHsc")
        .define("NDEBUG", None)
        .warnings(false)
        .include("vendor/scintilla/include")
        .include("vendor/scintilla/src")
        .include("vendor/lexilla/include")
        .include("vendor/lexilla/lexlib")
        .file("csrc/lexers.cxx");
    for dir in dirs {
        build.files(sources(dir));
    }
    build.compile("scintilla");

    for lib in LIBS {
        println!("cargo:rustc-link-lib={lib}");
    }
    // Controlli comuni v6 (temi di sistema) senza file .manifest separato.
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
    for dir in ["vendor", "csrc"] {
        println!("cargo:rerun-if-changed={}", Path::new(dir).display());
    }
}

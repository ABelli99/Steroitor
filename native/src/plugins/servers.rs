//! Dove trovare i server e come avviarli: jdtls (come nella versione Tauri) e tsserver con Node.
use std::path::{Path, PathBuf};
use std::process::Command;

const JAVA_MARKERS: [&str; 5] = ["pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle", ".project"];
const SCRIPT_MARKERS: [&str; 3] = ["tsconfig.json", "jsconfig.json", "package.json"];
const VSCODE_EXTENSION_DIRS: [&str; 3] = [".vscode/extensions", ".vscode-insiders/extensions", ".cursor/extensions"];
const TSSERVER: &str = "node_modules/typescript/lib/tsserver.js";

/// La cartella più vicina con uno dei marcatori; altrimenti quella del file.
pub fn project_root(file: &Path, markers: &[&str]) -> PathBuf {
    let start = file.parent().unwrap_or(file);
    start.ancestors().find(|dir| markers.iter().any(|marker| dir.join(marker).exists())).unwrap_or(start).to_path_buf()
}

pub fn java_root(file: &Path) -> PathBuf {
    project_root(file, &JAVA_MARKERS)
}

pub fn script_root(file: &Path) -> PathBuf {
    project_root(file, &SCRIPT_MARKERS)
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

fn first_match(dir: &Path, accept: impl Fn(&str) -> bool) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|entry| entry.path()).filter(|path| path.file_name().and_then(|name| name.to_str()).is_some_and(&accept)).max()
}

/// L'estensione Java di VS Code (redhat.java) contiene sia jdtls sia un JRE abbastanza recente.
fn vscode_java_extension() -> Option<PathBuf> {
    let home = home()?;
    VSCODE_EXTENSION_DIRS
        .iter()
        .filter_map(|dir| first_match(&home.join(dir), |name| name.starts_with("redhat.java-")))
        .filter(|path| path.join("server").is_dir())
        .max()
}

/// jdtls da `JDTLS_HOME` o dall'estensione di VS Code; Java da `JDTLS_JAVA_HOME`, dal JRE dell'estensione, da `JAVA_HOME` o dal PATH.
/// Il workspace di jdtls è lo stesso della versione Tauri: un progetto già importato non si reimporta.
pub fn jdtls(root: &Path, data_dir: &Path) -> Result<Command, String> {
    let extension = vscode_java_extension();
    let home = std::env::var_os("JDTLS_HOME").map(PathBuf::from).or_else(|| extension.as_ref().map(|dir| dir.join("server")));
    let home = home.ok_or("jdtls non trovato: imposta JDTLS_HOME o installa l'estensione Java di VS Code (redhat.java)")?;
    let launcher = first_match(&home.join("plugins"), |name| name.starts_with("org.eclipse.equinox.launcher_") && name.ends_with(".jar"))
        .ok_or_else(|| format!("launcher di jdtls non trovato in {}", home.display()))?;
    let bundled = extension.as_ref().and_then(|dir| first_match(&dir.join("jre"), |_| true)).map(|jre| jre.join("bin").join("java.exe"));
    let java = std::env::var_os("JDTLS_JAVA_HOME")
        .map(|dir| PathBuf::from(dir).join("bin").join("java.exe"))
        .or(bundled.filter(|path| path.exists()))
        .or_else(|| std::env::var_os("JAVA_HOME").map(|dir| PathBuf::from(dir).join("bin").join("java.exe")))
        .unwrap_or_else(|| PathBuf::from("java.exe"));

    let base = data_dir.join("jdtls");
    let key = root.to_string_lossy().to_lowercase();
    let fingerprint = key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
    let workspace = base.join("workspaces").join(format!("{fingerprint:016x}"));
    std::fs::create_dir_all(&workspace).map_err(|error| error.to_string())?;
    let mut command = Command::new(java);
    command
        .args([
            "-Declipse.application=org.eclipse.jdt.ls.core.id1",
            "-Dosgi.bundles.defaultStartLevel=4",
            "-Declipse.product=org.eclipse.jdt.ls.core.product",
            "-Dosgi.checkConfiguration=true",
            "-Dosgi.sharedConfiguration.area.readOnly=true",
            "-Dosgi.configuration.cascaded=true",
            "-Xms64m",
            "-Xmx1G",
            "-XX:+UseSerialGC",
            "--add-modules=ALL-SYSTEM",
            "--add-opens",
            "java.base/java.util=ALL-UNNAMED",
            "--add-opens",
            "java.base/java.lang=ALL-UNNAMED",
        ])
        .arg(format!("-Dosgi.sharedConfiguration.area={}", home.join("config_win").display()))
        .arg("-jar")
        .arg(launcher)
        .arg("-configuration")
        .arg(base.join("config"))
        .arg("-data")
        .arg(workspace)
        .current_dir(root);
    Ok(command)
}

/// tsserver.js: quello del progetto (node_modules), poi quello installato con `npm i -g typescript`, poi quello di VS Code.
fn locate_tsserver(root: &Path) -> Option<PathBuf> {
    let project = root.ancestors().map(|dir| dir.join(TSSERVER));
    let global = std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("npm").join(TSSERVER));
    let vscode = ["ProgramFiles", "LOCALAPPDATA"].into_iter().filter_map(std::env::var_os).flat_map(|dir| {
        let dir = PathBuf::from(dir);
        [dir.join("Microsoft VS Code"), dir.join("Programs").join("Microsoft VS Code")]
    });
    project.chain(global).chain(vscode.map(|dir| dir.join("resources").join("app").join("extensions").join(TSSERVER))).find(|path| path.is_file())
}

pub fn tsserver(root: &Path) -> Result<Command, String> {
    let script = locate_tsserver(root).ok_or("tsserver non trovato: installa TypeScript nel progetto o con `npm i -g typescript`")?;
    let mut command = Command::new("node");
    command.arg(script).args(["--disableAutomaticTypingAcquisition", "--locale", "it"]).current_dir(root);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_nearest_project_root() {
        let dir = std::env::temp_dir().join(format!("steroitor-plugins-{}", std::process::id()));
        let source = dir.join("app").join("src").join("main");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(dir.join("app").join("pom.xml"), "").unwrap();
        assert_eq!(java_root(&source.join("Main.java")), dir.join("app"));
        assert_eq!(script_root(&source.join("a.ts")), source);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

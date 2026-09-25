use portable_pty::CommandBuilder;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Shell {
    pub id: String,
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
}

impl Shell {
    fn new(id: &str, name: &str, program: impl AsRef<Path>, args: &[&str]) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            program: program.as_ref().to_string_lossy().into_owned(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
        }
    }

    pub fn command(&self) -> CommandBuilder {
        let mut command = CommandBuilder::new(&self.program);
        command.args(&self.args);
        command
    }
}

#[cfg(target_os = "windows")]
fn find_in_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).map(|dir| dir.join(name)).find(|candidate| candidate.is_file())
}

#[cfg(any(target_os = "windows", test))]
fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

#[cfg(any(target_os = "windows", test))]
/// `bash.exe` dell'installazione di Git for Windows che contiene `git` (`<root>\cmd\git.exe`).
fn git_bash_beside(git: &Path) -> Option<PathBuf> {
    existing(git.parent()?.parent()?.join("bin").join("bash.exe"))
}

#[cfg(target_os = "windows")]
fn detect() -> Vec<Shell> {
    let env_dir = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let system32 = env_dir("SystemRoot").unwrap_or_else(|| PathBuf::from(r"C:\Windows")).join("System32");
    let program_files = env_dir("ProgramFiles").unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));

    let pwsh = find_in_path("pwsh.exe").or_else(|| existing(program_files.join(r"PowerShell\7\pwsh.exe")));
    let powershell = existing(system32.join(r"WindowsPowerShell\v1.0\powershell.exe")).or_else(|| find_in_path("powershell.exe"));
    let cmd = env_dir("ComSpec").and_then(existing).or_else(|| existing(system32.join("cmd.exe")));
    let git_bash = find_in_path("git.exe")
        .and_then(|git| git_bash_beside(&git))
        .or_else(|| existing(program_files.join(r"Git\bin\bash.exe")));
    let wsl = existing(system32.join("wsl.exe"));

    [
        pwsh.map(|path| Shell::new("pwsh", "PowerShell 7", path, &["-NoLogo"])),
        powershell.map(|path| Shell::new("powershell", "Windows PowerShell", path, &["-NoLogo"])),
        cmd.map(|path| Shell::new("cmd", "Prompt dei comandi", path, &[])),
        git_bash.map(|path| Shell::new("git-bash", "Git Bash", path, &["--login", "-i"])),
        wsl.map(|path| Shell::new("wsl", "WSL", path, &[])),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(not(target_os = "windows"))]
fn detect() -> Vec<Shell> {
    let listed = std::fs::read_to_string("/etc/shells").unwrap_or_default();
    let login = std::env::var("SHELL").ok();
    let mut paths: Vec<String> = login.into_iter().collect();
    for line in listed.lines().map(str::trim).filter(|line| line.starts_with('/')) {
        if !paths.iter().any(|path| path == line) && Path::new(line).is_file() {
            paths.push(line.to_string());
        }
    }
    if paths.is_empty() {
        paths.push("/bin/sh".into());
    }
    paths
        .iter()
        .map(|path| {
            let name = Path::new(path).file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
            Shell::new(path, &name, path, &[])
        })
        .collect()
}

/// La shell scelta se ancora installata, altrimenti la prima rilevata (PowerShell 7 se c'è, su Windows).
fn choose(shells: Vec<Shell>, id: Option<&str>) -> Option<Shell> {
    let index = id.and_then(|id| shells.iter().position(|shell| shell.id == id)).unwrap_or(0);
    shells.into_iter().nth(index)
}

pub fn resolve(id: Option<&str>) -> Result<Shell, String> {
    choose(detect(), id).ok_or_else(|| "Nessuna shell trovata".into())
}

#[tauri::command]
pub fn terminal_shells() -> Vec<Shell> {
    detect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shells() -> Vec<Shell> {
        vec![Shell::new("pwsh", "PowerShell 7", "pwsh.exe", &[]), Shell::new("cmd", "Prompt dei comandi", "cmd.exe", &[])]
    }

    #[test]
    fn uses_the_chosen_shell_when_installed() {
        assert_eq!(choose(shells(), Some("cmd")).unwrap().id, "cmd");
    }

    #[test]
    fn falls_back_to_the_first_shell() {
        assert_eq!(choose(shells(), None).unwrap().id, "pwsh");
        assert_eq!(choose(shells(), Some("uninstalled")).unwrap().id, "pwsh");
        assert!(choose(Vec::new(), None).is_none());
    }

    #[test]
    fn finds_git_bash_next_to_git() {
        let root = std::env::temp_dir().join(format!("steroitor-git-bash-{}", std::process::id()));
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(root.join("bin").join("bash.exe"), "").unwrap();

        assert_eq!(git_bash_beside(&root.join("cmd").join("git.exe")), Some(root.join("bin").join("bash.exe")));
        assert_eq!(git_bash_beside(&root.join("missing").join("x").join("git.exe")), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_at_least_one_shell() {
        assert!(!detect().is_empty());
    }
}

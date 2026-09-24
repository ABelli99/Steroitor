use serde::Serialize;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

use crate::files::{decode, TextFile};

const CONSOLE_OUTPUT_LIMIT: usize = 16 * 1024;
const SETTINGS_FILE: &str = "settings.json";

/// Percorso dell'eseguibile git: `gitPath` in settings.json, altrimenti `git` dal PATH.
pub struct GitConfig {
    executable: RwLock<String>,
    next_id: AtomicU64,
}

impl GitConfig {
    pub fn load(app: &AppHandle) -> Self {
        let configured = app
            .path()
            .app_data_dir()
            .ok()
            .and_then(|dir| std::fs::read_to_string(dir.join(SETTINGS_FILE)).ok())
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|settings| settings.get("gitPath")?.as_str().map(str::to_owned));
        Self { executable: RwLock::new(configured.unwrap_or_else(|| "git".into())), next_id: AtomicU64::new(1) }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CommandLog {
    id: u64,
    command: String,
    cwd: String,
    code: Option<i32>,
    duration_ms: u128,
    started_at: u128,
    stdout: String,
    stderr: String,
    background: bool,
}

struct Output {
    stdout: Vec<u8>,
    stderr: String,
    success: bool,
}

fn truncate(text: &str) -> String {
    if text.len() <= CONSOLE_OUTPUT_LIMIT {
        return text.to_owned();
    }
    let mut end = CONSOLE_OUTPUT_LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… (output troncato, {} byte totali)", &text[..end], text.len())
}

fn quote(arg: &str) -> String {
    if arg.is_empty() || arg.contains([' ', '"', '\t']) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_owned()
    }
}

/// Esegue git e registra il comando nella Console (evento `git-command`).
fn run(app: &AppHandle, cwd: &Path, args: &[&str], background: bool) -> Result<Output, String> {
    let config = app.state::<GitConfig>();
    let executable = config.executable.read().map_err(|e| e.to_string())?.clone();
    let mut command = Command::new(&executable);
    command.args(args).current_dir(cwd).env("GIT_OPTIONAL_LOCKS", "0").env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let started_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let started = Instant::now();
    let result = command.output();
    let duration_ms = started.elapsed().as_millis();

    let (output, log_stdout, log_stderr, code) = match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).replace('\0', "\u{2400}");
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let code = output.status.code();
            let parsed = Output { stdout: output.stdout, stderr: stderr.clone(), success: output.status.success() };
            (Ok(parsed), stdout, stderr, code)
        }
        Err(error) => {
            let message = format!("Impossibile eseguire \"{executable}\": {error}");
            (Err(message.clone()), String::new(), message, None)
        }
    };

    let _ = app.emit(
        "git-command",
        CommandLog {
            id: config.next_id.fetch_add(1, Ordering::Relaxed),
            command: std::iter::once("git").chain(args.iter().copied()).map(quote).collect::<Vec<_>>().join(" "),
            cwd: cwd.to_string_lossy().into_owned(),
            code,
            duration_ms,
            started_at,
            stdout: truncate(&log_stdout),
            stderr: truncate(&log_stderr),
            background,
        },
    );
    output
}

fn run_text(app: &AppHandle, cwd: &Path, args: &[&str], background: bool) -> Result<String, String> {
    let output = run(app, cwd, args, background)?;
    if !output.success {
        return Err(output.stderr.trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn to_native(path: &str) -> String {
    if MAIN_SEPARATOR == '\\' {
        path.replace('/', "\\")
    } else {
        path.to_owned()
    }
}

fn native_path(root: &Path, relative: &str) -> String {
    to_native(&root.join(relative).to_string_lossy())
}

fn blocking<T: Send + 'static>(task: impl FnOnce() -> Result<T, String> + Send + 'static) -> impl std::future::Future<Output = Result<T, String>> {
    async move { tauri::async_runtime::spawn_blocking(task).await.map_err(|e| e.to_string())? }
}

// ---------- rilevamento ----------

#[tauri::command]
pub async fn git_repo_root(app: AppHandle, path: String) -> Result<Option<String>, String> {
    blocking(move || {
        let output = run(&app, Path::new(&path), &["rev-parse", "--show-toplevel"], true)?;
        if !output.success {
            return Ok(None);
        }
        Ok(Some(to_native(String::from_utf8_lossy(&output.stdout).trim())))
    })
    .await
}

// ---------- status ----------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    head: Option<String>,
    oid: Option<String>,
    upstream: Option<String>,
    ahead: u32,
    behind: u32,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Conflict,
    Untracked,
    Ignored,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    path: String,
    status: FileStatus,
    staged: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    branch: BranchInfo,
    files: Vec<StatusEntry>,
}

fn classify(xy: &str) -> FileStatus {
    let mut chars = xy.chars();
    let index = chars.next().unwrap_or('.');
    let worktree = chars.next().unwrap_or('.');
    if index == 'A' && worktree != 'D' {
        return FileStatus::Added;
    }
    if index == 'D' || worktree == 'D' {
        return FileStatus::Deleted;
    }
    FileStatus::Modified
}

fn parse_status(root: &Path, raw: &str) -> RepoStatus {
    let mut branch = BranchInfo::default();
    let mut files = Vec::new();
    let mut records = raw.split('\0').filter(|record| !record.is_empty());

    while let Some(record) = records.next() {
        let entry = |relative: &str, status, staged| StatusEntry { path: native_path(root, relative), status, staged };
        match record.split_once(' ') {
            Some(("#", header)) => match header.split_once(' ') {
                Some(("branch.oid", oid)) if oid != "(initial)" => branch.oid = Some(oid.into()),
                Some(("branch.head", head)) if head != "(detached)" => branch.head = Some(head.into()),
                Some(("branch.upstream", upstream)) => branch.upstream = Some(upstream.into()),
                Some(("branch.ab", counts)) => {
                    for count in counts.split(' ') {
                        let value = count[1..].parse().unwrap_or(0);
                        if count.starts_with('+') { branch.ahead = value } else { branch.behind = value }
                    }
                }
                _ => {}
            },
            Some(("1", rest)) => {
                let fields: Vec<&str> = rest.splitn(8, ' ').collect();
                if let [xy, .., path] = fields.as_slice() {
                    files.push(entry(path, classify(xy), !xy.starts_with('.')));
                }
            }
            Some(("2", rest)) => {
                let fields: Vec<&str> = rest.splitn(9, ' ').collect();
                if let [xy, .., path] = fields.as_slice() {
                    files.push(entry(path, classify(xy), !xy.starts_with('.')));
                }
                records.next();
            }
            Some(("u", rest)) => {
                if let Some(path) = rest.splitn(10, ' ').nth(9) {
                    files.push(entry(path, FileStatus::Conflict, false));
                }
            }
            Some(("?", path)) => files.push(entry(path, FileStatus::Untracked, false)),
            Some(("!", path)) => files.push(entry(path, FileStatus::Ignored, false)),
            _ => {}
        }
    }
    RepoStatus { branch, files }
}

#[tauri::command]
pub async fn git_status(app: AppHandle, root: String) -> Result<RepoStatus, String> {
    blocking(move || {
        let raw = run_text(&app, Path::new(&root), &["status", "--porcelain=v2", "--branch", "-z", "--ignored=matching"], true)?;
        Ok(parse_status(Path::new(&root), &raw))
    })
    .await
}

// ---------- branch e log ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    name: String,
    remote: bool,
    current: bool,
    upstream: Option<String>,
    hash: String,
}

fn parse_branches(raw: &str) -> Vec<Branch> {
    raw.lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let refname = fields.next()?;
            let hash = fields.next()?.to_owned();
            let current = fields.next()? == "*";
            let upstream = fields.next().filter(|u| !u.is_empty()).map(str::to_owned);
            let (name, remote) = match (refname.strip_prefix("refs/heads/"), refname.strip_prefix("refs/remotes/")) {
                (Some(local), _) => (local, false),
                (_, Some(remote)) if !remote.ends_with("/HEAD") => (remote, true),
                _ => return None,
            };
            Some(Branch { name: name.to_owned(), remote, current, upstream, hash })
        })
        .collect()
}

#[tauri::command]
pub async fn git_branches(app: AppHandle, root: String) -> Result<Vec<Branch>, String> {
    blocking(move || {
        let format = "--format=%(refname)%00%(objectname:short)%00%(HEAD)%00%(upstream:short)";
        let raw = run_text(&app, Path::new(&root), &["for-each-ref", format, "refs/heads", "refs/remotes"], true)?;
        Ok(parse_branches(&raw))
    })
    .await
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    hash: String,
    short_hash: String,
    author: String,
    email: String,
    timestamp: i64,
    parents: Vec<String>,
    refs: Vec<String>,
    subject: String,
    body: String,
}

const LOG_FORMAT: &str = "--format=%H%x00%h%x00%an%x00%ae%x00%at%x00%P%x00%D%x00%s%x00%b%x1e";

fn parse_log(raw: &str) -> Vec<Commit> {
    raw.split('\x1e')
        .filter_map(|record| {
            let fields: Vec<&str> = record.trim_start_matches('\n').split('\0').collect();
            let [hash, short_hash, author, email, timestamp, parents, refs, subject, body] = fields.as_slice() else {
                return None;
            };
            Some(Commit {
                hash: hash.to_string(),
                short_hash: short_hash.to_string(),
                author: author.to_string(),
                email: email.to_string(),
                timestamp: timestamp.parse().unwrap_or(0),
                parents: parents.split_whitespace().map(str::to_owned).collect(),
                refs: refs.split(", ").filter(|r| !r.is_empty()).map(str::to_owned).collect(),
                subject: subject.to_string(),
                body: body.trim_end().to_string(),
            })
        })
        .collect()
}

#[tauri::command]
pub async fn git_log(app: AppHandle, root: String, reference: Option<String>, skip: u32, limit: u32) -> Result<Vec<Commit>, String> {
    blocking(move || {
        let skip = format!("--skip={skip}");
        let limit = format!("--max-count={limit}");
        let target = reference.unwrap_or_else(|| "--all".into());
        let args = ["log", LOG_FORMAT, "--date-order", &skip, &limit, &target, "--"];
        match run_text(&app, Path::new(&root), &args, false) {
            Ok(raw) => Ok(parse_log(&raw)),
            Err(error) if error.contains("does not have any commits") || error.contains("unknown revision") => Ok(vec![]),
            Err(error) => Err(error),
        }
    })
    .await
}

// ---------- contenuto a HEAD (baseline per il gutter) ----------

#[tauri::command]
pub async fn git_head_content(app: AppHandle, root: String, path: String) -> Result<Option<TextFile>, String> {
    blocking(move || {
        let relative = PathBuf::from(&path);
        let relative = relative.strip_prefix(&root).map_err(|_| format!("{path} non è nel repository"))?;
        let spec = format!("HEAD:{}", relative.to_string_lossy().replace('\\', "/"));
        let output = run(&app, Path::new(&root), &["show", &spec], true)?;
        Ok(output.success.then(|| decode(&output.stdout)))
    })
    .await
}

#[tauri::command]
pub async fn git_version(app: AppHandle) -> Result<String, String> {
    blocking(move || run_text(&app, &std::env::temp_dir(), &["--version"], true).map(|v| v.trim().to_owned())).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_v2_status() {
        let root = Path::new("/repo");
        let raw = [
            "# branch.oid 1234abcd",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
            "1 .M N... 100644 100644 100644 aaa bbb src/app.ts",
            "1 A. N... 000000 100644 100644 000 ccc src/new file.ts",
            "2 R. N... 100644 100644 100644 ddd eee R100 src/renamed.ts",
            "src/old.ts",
            "u UU N... 100644 100644 100644 100644 f1 f2 f3 conflict.txt",
            "? notes.md",
            "! node_modules/",
            "",
        ]
        .join("\0");

        let status = parse_status(root, &raw);
        assert_eq!(status.branch.head.as_deref(), Some("main"));
        assert_eq!(status.branch.upstream.as_deref(), Some("origin/main"));
        assert_eq!((status.branch.ahead, status.branch.behind), (2, 1));

        let summary: Vec<(String, &FileStatus, bool)> = status
            .files
            .iter()
            .map(|f| (f.path.replace('\\', "/"), &f.status, f.staged))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("/repo/src/app.ts".into(), &FileStatus::Modified, false),
                ("/repo/src/new file.ts".into(), &FileStatus::Added, true),
                ("/repo/src/renamed.ts".into(), &FileStatus::Modified, true),
                ("/repo/conflict.txt".into(), &FileStatus::Conflict, false),
                ("/repo/notes.md".into(), &FileStatus::Untracked, false),
                ("/repo/node_modules/".into(), &FileStatus::Ignored, false),
            ]
        );
    }

    #[test]
    fn handles_detached_head_and_initial_commit() {
        let status = parse_status(Path::new("/r"), "# branch.oid (initial)\0# branch.head (detached)\0");
        assert!(status.branch.head.is_none());
        assert!(status.branch.oid.is_none());
    }

    #[test]
    fn parses_branches_skipping_remote_head() {
        let raw = "refs/heads/main\0abc1234\0*\0origin/main\nrefs/heads/feature/x\0def5678\0 \0\nrefs/remotes/origin/HEAD\0abc1234\0 \0\nrefs/remotes/origin/main\0abc1234\0 \0\n";
        let branches = parse_branches(raw);
        let names: Vec<(&str, bool, bool)> = branches.iter().map(|b| (b.name.as_str(), b.remote, b.current)).collect();
        assert_eq!(names, vec![("main", false, true), ("feature/x", false, false), ("origin/main", true, false)]);
        assert_eq!(branches[0].upstream.as_deref(), Some("origin/main"));
    }

    #[test]
    fn parses_log_records_with_multiline_bodies() {
        let raw = "aaa\0a\0Ada\0ada@x.it\01700000000\0p1 p2\0HEAD -> main, origin/main\0Merge\0riga 1\nriga 2\n\x1e\nbbb\0b\0Bob\0bob@x.it\01690000000\0\0\0Initial\0\x1e\n";
        let commits = parse_log(raw);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].parents, vec!["p1", "p2"]);
        assert_eq!(commits[0].refs, vec!["HEAD -> main", "origin/main"]);
        assert_eq!(commits[0].body, "riga 1\nriga 2");
        assert!(commits[1].parents.is_empty() && commits[1].refs.is_empty());
    }
}

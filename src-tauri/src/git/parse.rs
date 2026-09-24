use serde::Serialize;
use std::path::Path;

use super::runner::native_path;

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub head: Option<String>,
    pub oid: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Serialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Conflict,
    Untracked,
    Ignored,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    pub path: String,
    pub status: FileStatus,
    /// Ci sono modifiche nell'index (verranno committate).
    pub staged: bool,
    /// Ci sono modifiche nel working tree non ancora nell'index.
    pub unstaged: bool,
}

/// Operazione lasciata a metà da un conflitto: si conclude con continue o abort.
#[derive(Serialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub branch: BranchInfo,
    pub files: Vec<StatusEntry>,
    pub operation: Option<Operation>,
}

fn classify(xy: &str) -> (FileStatus, bool, bool) {
    let mut chars = xy.chars();
    let index = chars.next().unwrap_or('.');
    let worktree = chars.next().unwrap_or('.');
    let status = match (index, worktree) {
        ('A', worktree) if worktree != 'D' => FileStatus::Added,
        ('D', _) | (_, 'D') => FileStatus::Deleted,
        _ => FileStatus::Modified,
    };
    (status, index != '.', worktree != '.')
}

pub fn parse_status(root: &Path, raw: &str) -> RepoStatus {
    let mut branch = BranchInfo::default();
    let mut files = Vec::new();
    let mut records = raw.split('\0').filter(|record| !record.is_empty());

    while let Some(record) = records.next() {
        let tracked = |relative: &str, xy: &str| {
            let (status, staged, unstaged) = classify(xy);
            StatusEntry { path: native_path(root, relative), status, staged, unstaged }
        };
        let other = |relative: &str, status| StatusEntry { path: native_path(root, relative), status, staged: false, unstaged: true };

        match record.split_once(' ') {
            Some(("#", header)) => match header.split_once(' ') {
                Some(("branch.oid", oid)) if oid != "(initial)" => branch.oid = Some(oid.into()),
                Some(("branch.head", head)) if head != "(detached)" => branch.head = Some(head.into()),
                Some(("branch.upstream", upstream)) => branch.upstream = Some(upstream.into()),
                Some(("branch.ab", counts)) => {
                    for count in counts.split(' ') {
                        let value = count[1..].parse().unwrap_or(0);
                        if count.starts_with('+') {
                            branch.ahead = value;
                        } else {
                            branch.behind = value;
                        }
                    }
                }
                _ => {}
            },
            Some(("1", rest)) => {
                let fields: Vec<&str> = rest.splitn(8, ' ').collect();
                if let [xy, .., path] = fields.as_slice() {
                    files.push(tracked(path, xy));
                }
            }
            Some(("2", rest)) => {
                let fields: Vec<&str> = rest.splitn(9, ' ').collect();
                if let [xy, .., path] = fields.as_slice() {
                    files.push(tracked(path, xy));
                }
                records.next();
            }
            Some(("u", rest)) => {
                if let Some(path) = rest.splitn(10, ' ').nth(9) {
                    files.push(other(path, FileStatus::Conflict));
                }
            }
            Some(("?", path)) => files.push(other(path, FileStatus::Untracked)),
            Some(("!", path)) => files.push(StatusEntry { unstaged: false, ..other(path, FileStatus::Ignored) }),
            _ => {}
        }
    }
    RepoStatus { branch, files, operation: None }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub remote: bool,
    pub current: bool,
    pub upstream: Option<String>,
    pub hash: String,
}

pub const BRANCH_FORMAT: &str = "--format=%(refname)%00%(objectname:short)%00%(HEAD)%00%(upstream:short)";

pub fn parse_branches(raw: &str) -> Vec<Branch> {
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

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub parents: Vec<String>,
    pub refs: Vec<String>,
    pub subject: String,
    pub body: String,
}

pub const LOG_FORMAT: &str = "--format=%H%x00%h%x00%an%x00%ae%x00%at%x00%P%x00%D%x00%s%x00%b%x1e";

pub fn parse_log(raw: &str) -> Vec<Commit> {
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

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub hash: String,
    pub author: String,
    pub timestamp: i64,
    pub summary: String,
}

const UNCOMMITTED: &str = "0000000000000000000000000000000000000000";

/// `git blame --porcelain`: i metadati di un commit compaiono solo la prima volta.
pub fn parse_blame(raw: &str) -> Vec<BlameLine> {
    let mut known: std::collections::HashMap<String, BlameLine> = std::collections::HashMap::new();
    let mut lines = Vec::new();
    let mut current: Option<BlameLine> = None;

    for line in raw.lines() {
        if line.starts_with('\t') {
            if let Some(entry) = current.take() {
                known.entry(entry.hash.clone()).or_insert_with(|| entry.clone());
                lines.push(entry);
            }
            continue;
        }
        let Some(entry) = current.as_mut() else {
            let hash = line.split(' ').next().unwrap_or_default().to_owned();
            current = Some(known.get(&hash).cloned().unwrap_or(BlameLine {
                author: if hash == UNCOMMITTED { "Non committato".into() } else { String::new() },
                hash,
                timestamp: 0,
                summary: String::new(),
            }));
            continue;
        };
        match line.split_once(' ') {
            Some(("author", author)) if entry.hash != UNCOMMITTED => entry.author = author.to_owned(),
            Some(("author-time", time)) => entry.timestamp = time.parse().unwrap_or(0),
            Some(("summary", summary)) => entry.summary = summary.to_owned(),
            _ => {}
        }
    }
    lines
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stash {
    pub name: String,
    pub timestamp: i64,
    pub message: String,
}

pub const STASH_FORMAT: &str = "--format=%gd%x00%ct%x00%s";

pub fn parse_stashes(raw: &str) -> Vec<Stash> {
    raw.lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            Some(Stash {
                name: fields.next()?.to_owned(),
                timestamp: fields.next()?.parse().unwrap_or(0),
                message: fields.next().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blame_porcelain_reusing_commit_metadata() {
        let raw = "\
aaaa 1 1 2
author Ada
author-time 1700000000
summary Primo
filename a.txt
\tuno
aaaa 2 2
\tdue
0000000000000000000000000000000000000000 3 3 1
author Not Committed Yet
author-time 1800000000
summary Version of a.txt from a.txt
\ttre
";
        let lines = parse_blame(raw);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], BlameLine { hash: "aaaa".into(), author: "Ada".into(), timestamp: 1700000000, summary: "Primo".into() });
        assert_eq!(lines[2].author, "Non committato");
    }

    #[test]
    fn parses_stash_list() {
        let raw = "stash@{0}\x001700000000\x00On main: prova\nstash@{1}\x001600000000\x00WIP on main: abc\n";
        let stashes = parse_stashes(raw);
        assert_eq!(stashes[0], Stash { name: "stash@{0}".into(), timestamp: 1700000000, message: "On main: prova".into() });
        assert_eq!(stashes.len(), 2);
    }

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
            "1 MM N... 100644 100644 100644 aaa bbb src/both.ts",
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

        let summary: Vec<(String, FileStatus, bool, bool)> = status
            .files
            .iter()
            .map(|f| (f.path.replace('\\', "/"), f.status, f.staged, f.unstaged))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("/repo/src/app.ts".into(), FileStatus::Modified, false, true),
                ("/repo/src/new file.ts".into(), FileStatus::Added, true, false),
                ("/repo/src/both.ts".into(), FileStatus::Modified, true, true),
                ("/repo/src/renamed.ts".into(), FileStatus::Modified, true, false),
                ("/repo/conflict.txt".into(), FileStatus::Conflict, false, true),
                ("/repo/notes.md".into(), FileStatus::Untracked, false, true),
                ("/repo/node_modules/".into(), FileStatus::Ignored, false, false),
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

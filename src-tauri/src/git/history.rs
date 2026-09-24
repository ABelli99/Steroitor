use serde::Deserialize;
use std::path::Path;

use super::parse::{parse_blame, parse_log, parse_stashes, BlameLine, Commit, Operation, Stash, LOG_FORMAT, STASH_FORMAT};
use super::runner::{repo_relative, Git};

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogFilter {
    pub reference: Option<String>,
    pub author: Option<String>,
    pub text: Option<String>,
    pub path: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

const OPERATION_MARKERS: [(&str, Operation); 5] = [
    ("rebase-merge", Operation::Rebase),
    ("rebase-apply", Operation::Rebase),
    ("MERGE_HEAD", Operation::Merge),
    ("CHERRY_PICK_HEAD", Operation::CherryPick),
    ("REVERT_HEAD", Operation::Revert),
];

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|value| !value.is_empty())
}

impl Git {
    pub fn log_filtered(&self, root: &str, filter: &LogFilter, skip: u32, limit: u32) -> Result<Vec<Commit>, String> {
        let skip = format!("--skip={skip}");
        let limit = format!("--max-count={limit}");
        let author = non_empty(&filter.author).map(|author| format!("--author={author}"));
        let text = non_empty(&filter.text).map(|text| format!("--grep={text}"));

        let mut args = vec!["log", LOG_FORMAT, "--date-order", &skip, &limit];
        args.extend(author.as_deref());
        if let Some(text) = text.as_deref() {
            args.extend([text, "--regexp-ignore-case", "--fixed-strings"]);
        }
        args.push(non_empty(&filter.reference).unwrap_or("--all"));
        args.push("--");
        args.extend(non_empty(&filter.path));

        match self.text(Path::new(root), &args, false) {
            Ok(raw) => Ok(parse_log(&raw)),
            Err(error) if error.contains("does not have any commits") || error.contains("unknown revision") => Ok(vec![]),
            Err(error) => Err(error),
        }
    }

    pub fn operation(&self, root: &str) -> Result<Option<Operation>, String> {
        let mut args = vec!["rev-parse"];
        for (marker, _) in OPERATION_MARKERS {
            args.extend(["--git-path", marker]);
        }
        let paths = self.text(Path::new(root), &args, true)?;
        Ok(paths
            .lines()
            .zip(OPERATION_MARKERS)
            .find(|(path, _)| Path::new(root).join(path).exists())
            .map(|(_, (_, operation))| operation))
    }

    pub fn continue_operation(&self, root: &str) -> Result<(), String> {
        let args: &[&str] = match self.operation(root)? {
            Some(Operation::Merge) => &["commit", "--no-edit"],
            Some(Operation::Rebase) => &["rebase", "--continue"],
            Some(Operation::CherryPick) => &["cherry-pick", "--continue"],
            Some(Operation::Revert) => &["revert", "--continue"],
            None => return Err("Nessuna operazione in corso".into()),
        };
        self.text(Path::new(root), args, false).map(drop)
    }

    pub fn abort_operation(&self, root: &str) -> Result<(), String> {
        let command = match self.operation(root)? {
            Some(Operation::Merge) => "merge",
            Some(Operation::Rebase) => "rebase",
            Some(Operation::CherryPick) => "cherry-pick",
            Some(Operation::Revert) => "revert",
            None => return Err("Nessuna operazione in corso".into()),
        };
        self.text(Path::new(root), &[command, "--abort"], false).map(drop)
    }

    pub fn checkout_commit(&self, root: &str, hash: &str) -> Result<(), String> {
        self.text(Path::new(root), &["switch", "--detach", hash], false).map(drop)
    }

    pub fn cherry_pick(&self, root: &str, hash: &str) -> Result<(), String> {
        self.text(Path::new(root), &["cherry-pick", hash], false).map(drop)
    }

    pub fn revert(&self, root: &str, hash: &str) -> Result<(), String> {
        self.text(Path::new(root), &["revert", "--no-edit", hash], false).map(drop)
    }

    pub fn reset(&self, root: &str, hash: &str, mode: ResetMode) -> Result<(), String> {
        let mode = match mode {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        };
        self.text(Path::new(root), &["reset", mode, hash], false).map(drop)
    }

    pub fn create_tag(&self, root: &str, name: &str, hash: &str) -> Result<(), String> {
        self.text(Path::new(root), &["tag", name, hash], false).map(drop)
    }

    pub fn merge(&self, root: &str, branch: &str) -> Result<(), String> {
        self.text(Path::new(root), &["merge", "--no-edit", branch], false).map(drop)
    }

    pub fn rebase(&self, root: &str, onto: &str) -> Result<(), String> {
        self.text(Path::new(root), &["rebase", onto], false).map(drop)
    }

    pub fn blame(&self, root: &str, path: &str) -> Result<Vec<BlameLine>, String> {
        let relative = repo_relative(Path::new(root), path)?;
        let raw = self.text(Path::new(root), &["blame", "--porcelain", "--", &relative], false)?;
        Ok(parse_blame(&raw))
    }

    pub fn last_commit_message(&self, root: &str) -> Result<String, String> {
        self.text(Path::new(root), &["log", "-1", "--format=%B"], true).map(|message| message.trim_end().to_owned())
    }

    /// true se HEAD è già contenuto in almeno un branch remoto (amend riscriverebbe storia pubblicata).
    pub fn head_is_pushed(&self, root: &str) -> Result<bool, String> {
        let remotes = self.text(Path::new(root), &["branch", "-r", "--contains", "HEAD"], true)?;
        Ok(!remotes.trim().is_empty())
    }

    pub fn amend(&self, root: &str, message: &str) -> Result<(), String> {
        if message.trim().is_empty() {
            return Err("Il messaggio di commit è vuoto".into());
        }
        self.text(Path::new(root), &["commit", "--amend", "-m", message], false).map(drop)
    }

    pub fn stashes(&self, root: &str) -> Result<Vec<Stash>, String> {
        let raw = self.text(Path::new(root), &["stash", "list", STASH_FORMAT], true)?;
        Ok(parse_stashes(&raw))
    }

    pub fn stash_push(&self, root: &str, message: &str) -> Result<(), String> {
        let mut args = vec!["stash", "push", "--include-untracked"];
        if !message.trim().is_empty() {
            args.extend(["-m", message]);
        }
        self.text(Path::new(root), &args, false).map(drop)
    }

    pub fn stash_pop(&self, root: &str, name: &str) -> Result<(), String> {
        self.text(Path::new(root), &["stash", "pop", name], false).map(drop)
    }

    pub fn stash_drop(&self, root: &str, name: &str) -> Result<(), String> {
        self.text(Path::new(root), &["stash", "drop", name], false).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse::FileStatus;
    use super::super::testing::{commit_file, configure, git, status_of, write, Sandbox};
    use super::*;

    fn subjects(git: &Git, root: &str, filter: &LogFilter) -> Vec<String> {
        git.log_filtered(root, filter, 0, 50).unwrap().into_iter().map(|c| c.subject).collect()
    }

    fn head(git: &Git, root: &str) -> String {
        git.text(Path::new(root), &["rev-parse", "HEAD"], false).unwrap().trim().to_owned()
    }

    #[test]
    fn filters_log_by_author_text_and_path() {
        let sandbox = Sandbox::new("log-filter");
        let root = sandbox.repo("repo");
        let git = git();
        commit_file(&git, &root, "a.txt", "a\n", "Aggiunge A");
        configure(Path::new(&root), "Bruno", "bruno@example.com");
        commit_file(&git, &root, "b.txt", "b\n", "Fix su B");

        let author = LogFilter { author: Some("Bruno".into()), ..Default::default() };
        assert_eq!(subjects(&git, &root, &author), vec!["Fix su B"]);
        let text = LogFilter { text: Some("aggiunge".into()), ..Default::default() };
        assert_eq!(subjects(&git, &root, &text), vec!["Aggiunge A"]);
        let path = LogFilter { path: Some("b.txt".into()), ..Default::default() };
        assert_eq!(subjects(&git, &root, &path), vec!["Fix su B"]);
        let regex_chars = LogFilter { text: Some("[".into()), ..Default::default() };
        assert!(subjects(&git, &root, &regex_chars).is_empty(), "il testo è letterale, non una regex");
    }

    #[test]
    fn merge_conflict_can_be_aborted() {
        let sandbox = Sandbox::new("merge");
        let root = sandbox.repo("repo");
        let git = git();
        let file = commit_file(&git, &root, "a.txt", "base\n", "base");
        git.create_branch(&root, "feature", true, None).unwrap();
        commit_file(&git, &root, "a.txt", "feature\n", "feature");
        git.switch(&root, "main", false).unwrap();
        commit_file(&git, &root, "a.txt", "main\n", "main");

        assert!(git.merge(&root, "feature").is_err());
        assert_eq!(git.operation(&root).unwrap(), Some(Operation::Merge));
        assert_eq!(status_of(&git, &root, &file).map(|s| s.0), Some(FileStatus::Conflict));

        git.abort_operation(&root).unwrap();
        assert_eq!(git.operation(&root).unwrap(), None);
    }

    #[test]
    fn rebase_conflict_can_be_resolved_and_continued() {
        let sandbox = Sandbox::new("rebase");
        let root = sandbox.repo("repo");
        let git = git();
        commit_file(&git, &root, "a.txt", "base\n", "base");
        git.create_branch(&root, "feature", true, None).unwrap();
        commit_file(&git, &root, "a.txt", "feature\n", "feature");
        git.switch(&root, "main", false).unwrap();
        commit_file(&git, &root, "a.txt", "main\n", "main");
        git.switch(&root, "feature", false).unwrap();

        assert!(git.rebase(&root, "main").is_err());
        assert_eq!(git.operation(&root).unwrap(), Some(Operation::Rebase));

        let file = write(&root, "a.txt", "risolto\n");
        git.stage(&root, &[file]).unwrap();
        git.continue_operation(&root).unwrap();
        assert_eq!(git.operation(&root).unwrap(), None);
        assert_eq!(subjects(&git, &root, &LogFilter { reference: Some("feature".into()), ..Default::default() }), vec!["feature", "main", "base"]);
    }

    #[test]
    fn cherry_pick_revert_reset_and_tag() {
        let sandbox = Sandbox::new("history");
        let root = sandbox.repo("repo");
        let git = git();
        commit_file(&git, &root, "a.txt", "a\n", "base");
        let base = head(&git, &root);
        git.create_branch(&root, "feature", true, None).unwrap();
        commit_file(&git, &root, "b.txt", "b\n", "da prendere");
        let picked = head(&git, &root);
        git.switch(&root, "main", false).unwrap();

        git.cherry_pick(&root, &picked).unwrap();
        git.revert(&root, "HEAD").unwrap();
        let main = LogFilter { reference: Some("main".into()), ..Default::default() };
        assert_eq!(subjects(&git, &root, &main), vec!["Revert \"da prendere\"", "da prendere", "base"]);

        git.create_tag(&root, "v1", &base).unwrap();
        let tags = git.text(Path::new(&root), &["tag", "--points-at", &base], false).unwrap();
        assert_eq!(tags.trim(), "v1");

        git.reset(&root, &base, ResetMode::Soft).unwrap();
        assert_eq!(head(&git, &root), base);
        assert!(git.status(&root).unwrap().files.iter().all(|file| file.staged), "soft lascia le modifiche in stage");
        git.reset(&root, &base, ResetMode::Hard).unwrap();
        assert!(git.status(&root).unwrap().files.is_empty());

        git.checkout_commit(&root, &picked).unwrap();
        assert!(git.status(&root).unwrap().branch.head.is_none(), "detached HEAD");
        git.create_branch(&root, "dal-commit", false, Some(&base)).unwrap();
        let branch_head = git.text(Path::new(&root), &["rev-parse", "dal-commit"], false).unwrap();
        assert_eq!(branch_head.trim(), base);
    }

    #[test]
    fn blames_lines_including_uncommitted_ones() {
        let sandbox = Sandbox::new("blame");
        let root = sandbox.repo("repo");
        let git = git();
        let file = commit_file(&git, &root, "a.txt", "uno\ndue\n", "primo");
        configure(Path::new(&root), "Bruno", "bruno@example.com");
        commit_file(&git, &root, "a.txt", "uno\nDUE\n", "secondo");
        write(&root, "a.txt", "uno\nDUE\ntre\n");

        let lines: Vec<(String, String)> = git.blame(&root, &file).unwrap().into_iter().map(|l| (l.author, l.summary)).collect();
        assert_eq!(lines[0], ("Test".into(), "primo".into()));
        assert_eq!(lines[1], ("Bruno".into(), "secondo".into()));
        assert_eq!(lines[2].0, "Non committato");
    }

    #[test]
    fn amends_and_detects_pushed_head() {
        let sandbox = Sandbox::new("amend");
        let remote = sandbox.bare("remote.git");
        let root = sandbox.repo("repo");
        let git = git();
        git.text(Path::new(&root), &["remote", "add", "origin", &remote], false).unwrap();
        commit_file(&git, &root, "a.txt", "a\n", "messaggio sbagliato");
        assert_eq!(git.last_commit_message(&root).unwrap(), "messaggio sbagliato");
        assert!(!git.head_is_pushed(&root).unwrap());

        git.amend(&root, "messaggio giusto").unwrap();
        assert_eq!(git.last_commit_message(&root).unwrap(), "messaggio giusto");
        assert_eq!(git.log_filtered(&root, &LogFilter::default(), 0, 10).unwrap().len(), 1);

        git.push(&root).unwrap();
        assert!(git.head_is_pushed(&root).unwrap());
    }

    #[test]
    fn stash_push_list_pop_and_drop() {
        let sandbox = Sandbox::new("stash");
        let root = sandbox.repo("repo");
        let git = git();
        let file = commit_file(&git, &root, "a.txt", "a\n", "base");
        write(&root, "a.txt", "modificato\n");
        let untracked = write(&root, "nuovo.txt", "x\n");

        git.stash_push(&root, "lavoro a metà").unwrap();
        assert!(git.status(&root).unwrap().files.is_empty(), "include anche gli untracked");
        let stashes = git.stashes(&root).unwrap();
        assert_eq!(stashes.len(), 1);
        assert!(stashes[0].message.ends_with("lavoro a metà"));

        git.stash_pop(&root, &stashes[0].name).unwrap();
        assert_eq!(status_of(&git, &root, &file).map(|s| s.0), Some(FileStatus::Modified));
        assert_eq!(status_of(&git, &root, &untracked).map(|s| s.0), Some(FileStatus::Untracked));

        git.stash_push(&root, "").unwrap();
        git.stash_drop(&root, "stash@{0}").unwrap();
        assert!(git.stashes(&root).unwrap().is_empty());
    }
}

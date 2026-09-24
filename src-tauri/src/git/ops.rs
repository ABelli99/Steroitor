use std::path::Path;

use super::parse::{parse_branches, parse_status, Branch, RepoStatus, BRANCH_FORMAT};
use super::runner::{repo_relative, to_native, Git};
use crate::files::{decode, TextFile};

const DEFAULT_REMOTE: &str = "origin";

impl Git {
    pub fn version(&self) -> Result<String, String> {
        self.text(&std::env::temp_dir(), &["--version"], true).map(|v| v.trim().to_owned())
    }

    pub fn repo_root(&self, path: &str) -> Result<Option<String>, String> {
        let output = self.run(Path::new(path), &["rev-parse", "--show-toplevel"], true)?;
        Ok(output.success.then(|| to_native(output.text().trim())))
    }

    pub fn status(&self, root: &str) -> Result<RepoStatus, String> {
        let raw = self.text(Path::new(root), &["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all", "--ignored=matching"], true)?;
        let mut status = parse_status(Path::new(root), &raw);
        status.operation = self.operation(root)?;
        Ok(status)
    }

    pub fn branches(&self, root: &str) -> Result<Vec<Branch>, String> {
        let raw = self.text(Path::new(root), &["for-each-ref", BRANCH_FORMAT, "refs/heads", "refs/remotes"], true)?;
        Ok(parse_branches(&raw))
    }

    pub fn head_content(&self, root: &str, path: &str) -> Result<Option<TextFile>, String> {
        let spec = format!("HEAD:{}", repo_relative(Path::new(root), path)?);
        let output = self.run(Path::new(root), &["show", &spec], true)?;
        Ok(output.success.then(|| decode(&output.stdout)))
    }


    pub fn stage(&self, root: &str, paths: &[String]) -> Result<(), String> {
        let relative = relative_paths(root, paths)?;
        let mut args = vec!["add", "-A", "--"];
        args.extend(relative.iter().map(String::as_str));
        self.text(Path::new(root), &args, false).map(drop)
    }

    pub fn unstage(&self, root: &str, paths: &[String]) -> Result<(), String> {
        let relative = relative_paths(root, paths)?;
        let has_head = self.run(Path::new(root), &["rev-parse", "--verify", "--quiet", "HEAD"], true)?.success;
        let mut args = if has_head { vec!["restore", "--staged", "--"] } else { vec!["rm", "--cached", "-r", "--quiet", "--"] };
        args.extend(relative.iter().map(String::as_str));
        self.text(Path::new(root), &args, false).map(drop)
    }

    pub fn commit(&self, root: &str, message: &str) -> Result<(), String> {
        if message.trim().is_empty() {
            return Err("Il messaggio di commit è vuoto".into());
        }
        self.text(Path::new(root), &["commit", "-m", message], false).map(drop)
    }

    /// Push del branch corrente; se manca l'upstream lo imposta sul remote predefinito.
    pub fn push(&self, root: &str) -> Result<(), String> {
        let cwd = Path::new(root);
        let has_upstream = self.run(cwd, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"], true)?.success;
        if has_upstream {
            return self.text(cwd, &["push"], false).map(drop);
        }
        let remote = self.default_remote(root)?;
        self.text(cwd, &["push", "--set-upstream", &remote, "HEAD"], false).map(drop)
    }

    pub fn pull(&self, root: &str, rebase: bool) -> Result<(), String> {
        let mode = if rebase { "--rebase" } else { "--no-rebase" };
        self.text(Path::new(root), &["pull", mode], false).map(drop)
    }

    pub fn fetch(&self, root: &str) -> Result<(), String> {
        self.text(Path::new(root), &["fetch", "--all", "--prune"], false).map(drop)
    }

    /// Checkout di un branch locale, o di un remoto creando il branch locale che lo traccia.
    pub fn switch(&self, root: &str, branch: &str, remote: bool) -> Result<(), String> {
        let args: &[&str] = if remote { &["switch", "--track", branch] } else { &["switch", branch] };
        self.text(Path::new(root), args, false).map(drop)
    }

    pub fn create_branch(&self, root: &str, name: &str, checkout: bool, start: Option<&str>) -> Result<(), String> {
        let mut args = if checkout { vec!["switch", "-c", name] } else { vec!["branch", name] };
        args.extend(start);
        self.text(Path::new(root), &args, false).map(drop)
    }

    /// File in stage con terminazioni CRLF quando core.autocrlf non le converte.
    pub fn staged_crlf_files(&self, root: &str) -> Result<Vec<String>, String> {
        let cwd = Path::new(root);
        let autocrlf = self.run(cwd, &["config", "--get", "core.autocrlf"], true)?.text();
        if matches!(autocrlf.trim(), "true" | "input") {
            return Ok(vec![]);
        }
        let staged = self.text(cwd, &["diff", "--cached", "--name-only", "-z", "--diff-filter=ACMR"], true)?;
        let files: Vec<&str> = staged.split('\0').filter(|file| !file.is_empty()).collect();
        if files.is_empty() {
            return Ok(vec![]);
        }
        let mut args = vec!["grep", "--cached", "-I", "-l", "-z", "-e", "\r", "--"];
        args.extend(files);
        let output = self.run(cwd, &args, true)?;
        Ok(output.text().split('\0').filter(|file| !file.is_empty()).map(str::to_owned).collect())
    }

    fn default_remote(&self, root: &str) -> Result<String, String> {
        let remotes = self.text(Path::new(root), &["remote"], true)?;
        let remotes: Vec<&str> = remotes.lines().filter(|line| !line.is_empty()).collect();
        remotes
            .iter()
            .find(|remote| **remote == DEFAULT_REMOTE)
            .or(remotes.first())
            .map(|remote| remote.to_string())
            .ok_or_else(|| "Nessun remote configurato".into())
    }
}

fn relative_paths(root: &str, paths: &[String]) -> Result<Vec<String>, String> {
    paths.iter().map(|path| repo_relative(Path::new(root), path)).collect()
}

#[cfg(test)]
mod tests {
    use super::super::parse::FileStatus;
    use super::super::history::LogFilter;
    use super::super::testing::{git, status_of, write, Sandbox};
    use super::*;
    use std::fs;

    #[test]
    fn stage_commit_and_unstage_round_trip() {
        let sandbox = Sandbox::new("commit");
        let root = sandbox.repo("repo");
        let git = git();
        let file = write(&root, "a.txt", "uno\n");

        assert_eq!(status_of(&git, &root, &file), Some((FileStatus::Untracked, false, true)));

        git.stage(&root, &[file.clone()]).unwrap();
        assert_eq!(status_of(&git, &root, &file), Some((FileStatus::Added, true, false)));

        git.unstage(&root, &[file.clone()]).unwrap();
        assert_eq!(status_of(&git, &root, &file), Some((FileStatus::Untracked, false, true)), "unstage senza HEAD");

        git.stage(&root, &[file.clone()]).unwrap();
        git.commit(&root, "primo commit").unwrap();
        assert_eq!(status_of(&git, &root, &file), None);

        let log = git.log_filtered(&root, &LogFilter::default(), 0, 10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].subject, "primo commit");
        assert_eq!(git.head_content(&root, &file).unwrap().map(|f| f.content), Some("uno\n".into()));

        write(&root, "a.txt", "due\n");
        git.stage(&root, &[file.clone()]).unwrap();
        git.unstage(&root, &[file.clone()]).unwrap();
        assert_eq!(status_of(&git, &root, &file), Some((FileStatus::Modified, false, true)), "unstage con HEAD");

        assert!(git.commit(&root, "   ").is_err());
    }

    #[test]
    fn lists_files_inside_untracked_directories() {
        let sandbox = Sandbox::new("untracked-dir");
        let root = sandbox.repo("repo");
        fs::create_dir_all(Path::new(&root).join("nuova")).unwrap();
        let file = write(&root, "nuova/file.txt", "x\n");
        assert_eq!(status_of(&git(), &root, &file.replace('/', "\\")), Some((FileStatus::Untracked, false, true)));
    }

    #[test]
    fn stages_deletions() {
        let sandbox = Sandbox::new("delete");
        let root = sandbox.repo("repo");
        let git = git();
        let file = write(&root, "gone.txt", "x\n");
        git.stage(&root, &[file.clone()]).unwrap();
        git.commit(&root, "init").unwrap();

        fs::remove_file(&file).unwrap();
        git.stage(&root, &[file.clone()]).unwrap();
        assert_eq!(status_of(&git, &root, &file), Some((FileStatus::Deleted, true, false)));
    }

    #[test]
    fn creates_and_switches_branches() {
        let sandbox = Sandbox::new("branches");
        let root = sandbox.repo("repo");
        let git = git();
        git.stage(&root, &[write(&root, "a.txt", "a\n")]).unwrap();
        git.commit(&root, "init").unwrap();

        git.create_branch(&root, "feature", true, None).unwrap();
        git.create_branch(&root, "other", false, None).unwrap();
        let current: Vec<String> = git.branches(&root).unwrap().into_iter().filter(|b| b.current).map(|b| b.name).collect();
        assert_eq!(current, vec!["feature"]);

        git.switch(&root, "main", false).unwrap();
        assert_eq!(git.status(&root).unwrap().branch.head.as_deref(), Some("main"));
        assert!(git.switch(&root, "missing", false).is_err());
    }

    #[test]
    fn push_sets_upstream_then_pull_brings_remote_commits() {
        let sandbox = Sandbox::new("remote");
        let remote = sandbox.bare("remote.git");
        let first = sandbox.repo("first");
        let git = git();
        git.text(Path::new(&first), &["remote", "add", "origin", &remote], false).unwrap();
        git.stage(&first, &[write(&first, "a.txt", "a\n")]).unwrap();
        git.commit(&first, "init").unwrap();

        git.push(&first).unwrap();
        assert_eq!(git.status(&first).unwrap().branch.upstream.as_deref(), Some("origin/main"));

        let second = sandbox.clone(&remote, "second");
        git.stage(&second, &[write(&second, "b.txt", "b\n")]).unwrap();
        git.commit(&second, "dal secondo clone").unwrap();
        git.push(&second).unwrap();

        git.fetch(&first).unwrap();
        assert_eq!(git.status(&first).unwrap().branch.behind, 1);
        git.pull(&first, false).unwrap();
        assert_eq!(git.log_filtered(&first, &LogFilter { reference: Some("main".into()), ..Default::default() }, 0, 1).unwrap()[0].subject, "dal secondo clone");
    }

    #[test]
    fn reports_staged_files_with_crlf_only_without_autocrlf() {
        let sandbox = Sandbox::new("crlf");
        let root = sandbox.repo("repo");
        let git = git();
        git.stage(&root, &[write(&root, "win.txt", "a\r\nb\r\n"), write(&root, "unix.txt", "a\nb\n")]).unwrap();
        assert_eq!(git.staged_crlf_files(&root).unwrap(), vec!["win.txt"]);

        git.text(Path::new(&root), &["config", "core.autocrlf", "true"], false).unwrap();
        assert!(git.staged_crlf_files(&root).unwrap().is_empty());
    }

    #[test]
    fn push_without_remote_explains_the_problem() {
        let sandbox = Sandbox::new("noremote");
        let root = sandbox.repo("repo");
        let git = git();
        git.stage(&root, &[write(&root, "a.txt", "a\n")]).unwrap();
        git.commit(&root, "init").unwrap();
        assert_eq!(git.push(&root).unwrap_err(), "Nessun remote configurato");
    }
}

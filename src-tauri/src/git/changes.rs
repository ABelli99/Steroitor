//! File cambiati in un commit e contenuto di un file a una revisione, per la vista delle modifiche.

use serde::Serialize;
use std::path::Path;

use super::runner::{native_path, repo_relative, Git};
use crate::files::decode;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    /// Lettera di `git diff-tree --name-status`: A, M, D, R, C, T.
    pub status: String,
}

/// Output `-z --name-status`: `status\0path\0`, oppure `Rnnn\0vecchio\0nuovo\0` per rename e copie.
fn parse_name_status(root: &Path, raw: &str) -> Vec<ChangedFile> {
    let mut fields = raw.split('\0').filter(|field| !field.is_empty());
    let mut files = Vec::new();
    while let Some(code) = fields.next() {
        let status = code[..1].to_owned();
        let Some(first) = fields.next() else { break };
        let (old_path, path) = if status == "R" || status == "C" {
            let Some(second) = fields.next() else { break };
            (Some(native_path(root, first)), native_path(root, second))
        } else {
            (None, native_path(root, first))
        };
        files.push(ChangedFile { path, old_path, status });
    }
    files
}

impl Git {
    /// File cambiati da `hash` rispetto al primo parent (o all'albero vuoto per il commit iniziale).
    pub fn commit_changes(&self, root: &str, hash: &str) -> Result<Vec<ChangedFile>, String> {
        let cwd = Path::new(root);
        let first_parent = format!("{hash}^1");
        let has_parent = self.run(cwd, &["rev-parse", "--verify", "--quiet", &first_parent], true)?.success;
        let mut args = vec!["diff-tree", "-r", "-M", "--no-commit-id", "--name-status", "-z"];
        if has_parent {
            args.extend([first_parent.as_str(), hash]);
        } else {
            args.extend(["--root", hash]);
        }
        let raw = self.text(cwd, &args, false)?;
        Ok(parse_name_status(Path::new(root), &raw))
    }

    /// Contenuto di `path` a `revision` (`None` se il file lì non esiste).
    pub fn file_at(&self, root: &str, revision: &str, path: &str) -> Result<Option<String>, String> {
        let spec = format!("{revision}:{}", repo_relative(Path::new(root), path)?);
        let output = self.run(Path::new(root), &["show", &spec], true)?;
        Ok(output.success.then(|| decode(&output.stdout).content))
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{commit_file, git, Sandbox};
    use super::*;

    #[test]
    fn parses_renames_and_plain_changes() {
        let files = parse_name_status(Path::new("/r"), "M\0a.txt\0R087\0old.txt\0new.txt\0D\0gone.txt\0");
        let summary: Vec<(String, Option<String>, String)> = files
            .into_iter()
            .map(|f| (f.path.replace('\\', "/"), f.old_path.map(|p| p.replace('\\', "/")), f.status))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("/r/a.txt".into(), None, "M".into()),
                ("/r/new.txt".into(), Some("/r/old.txt".into()), "R".into()),
                ("/r/gone.txt".into(), None, "D".into()),
            ]
        );
    }

    #[test]
    fn lists_changes_of_the_root_commit_and_of_later_ones() {
        let sandbox = Sandbox::new("changes");
        let root = sandbox.repo("repo");
        let git = git();
        let a = commit_file(&git, &root, "a.txt", "uno\n", "primo");
        let first = git.commit_changes(&root, "HEAD").unwrap();
        assert_eq!(first.iter().map(|f| (f.path.clone(), f.status.clone())).collect::<Vec<_>>(), vec![(a.clone(), "A".into())]);

        commit_file(&git, &root, "a.txt", "due\n", "secondo");
        let second = git.commit_changes(&root, "HEAD").unwrap();
        assert_eq!(second[0].status, "M");
        assert_eq!(git.file_at(&root, "HEAD^", &a).unwrap().as_deref(), Some("uno\n"));
        assert_eq!(git.file_at(&root, "HEAD", &a).unwrap().as_deref(), Some("due\n"));
        assert_eq!(git.file_at(&root, "HEAD", &format!("{root}{}manca.txt", std::path::MAIN_SEPARATOR)).unwrap(), None);
    }

    #[test]
    fn shows_merge_changes_against_the_first_parent() {
        let sandbox = Sandbox::new("changes-merge");
        let root = sandbox.repo("repo");
        let git = git();
        commit_file(&git, &root, "a.txt", "a\n", "base");
        git.create_branch(&root, "feature", true, None).unwrap();
        let feature_file = commit_file(&git, &root, "f.txt", "f\n", "feature");
        git.switch(&root, "main", false).unwrap();
        commit_file(&git, &root, "m.txt", "m\n", "main");
        git.merge(&root, "feature").unwrap();

        let merged = git.commit_changes(&root, "HEAD").unwrap();
        assert_eq!(merged.iter().map(|f| f.path.clone()).collect::<Vec<_>>(), vec![feature_file]);
    }
}

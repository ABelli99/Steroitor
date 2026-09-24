//! Staging parziale: il frontend calcola il nuovo contenuto dell'index (index + solo i hunk scelti)
//! e qui lo si scrive senza toccare il working tree.

use std::path::Path;

use super::runner::{repo_relative, Git};
use crate::files::decode;

const DEFAULT_MODE: &str = "100644";

impl Git {
    /// Contenuto del file nell'index (`None` se il file non è tracciato).
    pub fn index_content(&self, root: &str, path: &str) -> Result<Option<String>, String> {
        let spec = format!(":{}", repo_relative(Path::new(root), path)?);
        let output = self.run(Path::new(root), &["show", &spec], true)?;
        Ok(output.success.then(|| decode(&output.stdout).content))
    }

    /// Scrive `content` come versione in stage di `path`. `--path` applica i filtri (autocrlf, attributes).
    pub fn stage_content(&self, root: &str, path: &str, content: &str) -> Result<(), String> {
        let cwd = Path::new(root);
        let relative = repo_relative(cwd, path)?;
        let path_arg = format!("--path={relative}");
        let hashed = self.run_with_input(cwd, &["hash-object", "-w", &path_arg, "--stdin"], Some(content.as_bytes()), false)?;
        if !hashed.success {
            return Err(hashed.stderr.trim().to_owned());
        }
        let hash = hashed.text().trim().to_owned();

        let listed = self.text(cwd, &["ls-files", "-s", "--", &relative], true)?;
        let mode = listed.split_whitespace().next().unwrap_or(DEFAULT_MODE).to_owned();
        let cacheinfo = format!("{mode},{hash},{relative}");
        self.text(cwd, &["update-index", "--add", "--cacheinfo", &cacheinfo], false).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse::FileStatus;
    use super::super::testing::{commit_file, git, status_of, write, Sandbox};
    use std::path::Path;

    #[test]
    fn stages_only_the_given_content_leaving_the_working_tree_alone() {
        let sandbox = Sandbox::new("partial");
        let root = sandbox.repo("repo");
        let git = git();
        let file = commit_file(&git, &root, "a.txt", "uno\ndue\ntre\n", "base");
        write(&root, "a.txt", "UNO\ndue\nTRE\n");

        git.stage_content(&root, &file, "UNO\ndue\ntre\n").unwrap();

        assert_eq!(git.index_content(&root, &file).unwrap().as_deref(), Some("UNO\ndue\ntre\n"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "UNO\ndue\nTRE\n");
        assert_eq!(status_of(&git, &root, &file), Some((FileStatus::Modified, true, true)));
    }

    #[test]
    fn applies_autocrlf_when_staging_content() {
        let sandbox = Sandbox::new("partial-crlf");
        let root = sandbox.repo("repo");
        let git = git();
        git.text(Path::new(&root), &["config", "core.autocrlf", "true"], false).unwrap();
        let file = commit_file(&git, &root, "a.txt", "a\r\nb\r\n", "base");

        git.stage_content(&root, &file, "A\r\nb\r\n").unwrap();
        let staged = git.text(Path::new(&root), &["cat-file", "-p", ":a.txt"], false).unwrap();
        assert_eq!(staged, "A\nb\n", "l'index riceve LF come con git add");
    }

    #[test]
    fn stages_a_new_file_and_reports_untracked_as_missing_from_index() {
        let sandbox = Sandbox::new("partial-new");
        let root = sandbox.repo("repo");
        let git = git();
        let file = write(&root, "nuovo.txt", "x\ny\n");
        assert_eq!(git.index_content(&root, &file).unwrap(), None);

        git.stage_content(&root, &file, "x\n").unwrap();
        assert_eq!(git.index_content(&root, &file).unwrap().as_deref(), Some("x\n"));
        assert_eq!(status_of(&git, &root, &file), Some((FileStatus::Added, true, true)));
    }
}

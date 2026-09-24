//! Creazione di repository: clone da URL e git init.

use std::path::Path;

use super::runner::Git;

impl Git {
    /// Clona `url` in `target` (che non deve esistere o deve essere vuota).
    pub fn clone_repo(&self, url: &str, target: &str) -> Result<(), String> {
        let parent = Path::new(target).parent().ok_or("Percorso di destinazione non valido")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        self.text(parent, &["clone", "--progress", url, target], false).map(drop)
    }

    pub fn init_repo(&self, path: &str) -> Result<(), String> {
        self.text(Path::new(path), &["init"], false).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{commit_file, git, Sandbox};

    #[test]
    fn clones_a_repository_into_a_new_folder() {
        let sandbox = Sandbox::new("clone");
        let source = sandbox.repo("source");
        let git = git();
        commit_file(&git, &source, "a.txt", "a\n", "init");

        let target = sandbox.0.join("nested").join("copy").to_string_lossy().into_owned();
        git.clone_repo(&source, &target).unwrap();
        assert_eq!(std::fs::read_to_string(std::path::Path::new(&target).join("a.txt")).unwrap(), "a\n");
        assert_eq!(git.repo_root(&target).unwrap().map(|root| root.to_lowercase()), Some(target.to_lowercase()));
    }

    #[test]
    fn clone_of_a_missing_remote_reports_the_git_error() {
        let sandbox = Sandbox::new("clone-missing");
        let target = sandbox.0.join("copy").to_string_lossy().into_owned();
        let error = git().clone_repo(&sandbox.0.join("non-esiste").to_string_lossy(), &target).unwrap_err();
        assert!(error.to_lowercase().contains("does not exist") || error.contains("not found") || error.contains("fatal"), "{error}");
    }

    #[test]
    fn initialises_a_repository_in_an_existing_folder() {
        let sandbox = Sandbox::new("init");
        let folder = sandbox.0.join("plain");
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.to_string_lossy().into_owned();
        assert_eq!(git().repo_root(&path).unwrap(), None);
        git().init_repo(&path).unwrap();
        assert!(git().repo_root(&path).unwrap().is_some());
    }
}

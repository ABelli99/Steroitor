//! Repository git temporanei per i test di integrazione.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::parse::FileStatus;
use super::runner::Git;

pub fn git() -> Git {
    Git::new("git", Box::new(|_| {}))
}

pub struct Sandbox(pub PathBuf);

impl Sandbox {
    pub fn new(name: &str) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("steroitor-{name}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn repo(&self, name: &str) -> String {
        let path = self.0.join(name);
        fs::create_dir_all(&path).unwrap();
        git().text(&path, &["init", "-q", "-b", "main"], false).unwrap();
        configure(&path, "Test", "test@example.com");
        git().text(&path, &["config", "core.autocrlf", "false"], false).unwrap();
        path.to_string_lossy().into_owned()
    }

    pub fn bare(&self, name: &str) -> String {
        let path = self.0.join(name);
        git().text(&self.0, &["init", "-q", "--bare", "-b", "main", &path.to_string_lossy()], false).unwrap();
        path.to_string_lossy().into_owned()
    }

    pub fn clone(&self, remote: &str, name: &str) -> String {
        let path = self.0.join(name);
        git().text(&self.0, &["clone", "-q", remote, &path.to_string_lossy()], false).unwrap();
        configure(&path, "Other", "other@example.com");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn configure(repo: &Path, name: &str, email: &str) {
    for args in [["config", "user.name", name], ["config", "user.email", email], ["config", "commit.gpgsign", "false"]] {
        git().text(repo, &args, false).unwrap();
    }
}

pub fn write(root: &str, name: &str, content: &str) -> String {
    let path = Path::new(root).join(name);
    fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}

/// Scrive, mette in stage e committa un file; restituisce il percorso.
pub fn commit_file(git: &Git, root: &str, name: &str, content: &str, message: &str) -> String {
    let path = write(root, name, content);
    git.stage(root, &[path.clone()]).unwrap();
    git.commit(root, message).unwrap();
    path
}

pub fn status_of(git: &Git, root: &str, path: &str) -> Option<(FileStatus, bool, bool)> {
    let status = git.status(root).unwrap();
    status.files.iter().find(|f| f.path == path).map(|f| (f.status, f.staged, f.unstaged))
}

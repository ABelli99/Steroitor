use std::path::{Path, PathBuf};

/// Abbastanza per `gipso/gipso-fe` o `workspace/clienti/app`, senza scandire alberi enormi.
const MAX_DEPTH: usize = 3;
const SKIPPED: &[&str] = &["node_modules", "target", "dist", "build", "out", "vendor", "bin", "obj"];

/// Repository dentro `root` (lui compreso): cartelle con un `.git`, che sia una directory
/// o un file (submodule, worktree). Prosegue anche dentro i repository trovati, per i submodule.
pub fn find_repos(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, 0, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if dir.join(".git").exists() {
        found.push(dir.to_path_buf());
    }
    if depth == MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIPPED.contains(&name.as_ref()) {
            continue;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            walk(&entry.path(), depth + 1, found);
        }
    }
}

#[tauri::command]
pub async fn git_find_repos(root: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        find_repos(Path::new(&root)).into_iter().map(|path| path.to_string_lossy().into_owned()).collect()
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sandbox(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("steroitor-discover-{name}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn repo(path: &Path) {
        fs::create_dir_all(path.join(".git")).unwrap();
    }

    #[test]
    fn finds_sibling_repositories_under_a_plain_folder() {
        let root = sandbox("siblings");
        repo(&root.join("gipso-fe"));
        repo(&root.join("gipso-be"));
        fs::create_dir_all(root.join("docs")).unwrap();
        assert_eq!(find_repos(&root), vec![root.join("gipso-be"), root.join("gipso-fe")]);
    }

    #[test]
    fn includes_the_root_and_submodules_with_a_git_file() {
        let root = sandbox("submodule");
        repo(&root);
        fs::create_dir_all(root.join("libs").join("shared")).unwrap();
        fs::write(root.join("libs").join("shared").join(".git"), "gitdir: ../../.git/modules/shared").unwrap();
        assert_eq!(find_repos(&root), vec![root.clone(), root.join("libs").join("shared")]);
    }

    #[test]
    fn skips_dependencies_hidden_folders_and_deep_trees() {
        let root = sandbox("skipped");
        repo(&root.join("node_modules").join("pkg"));
        repo(&root.join(".cache").join("tool"));
        repo(&root.join("a").join("b").join("c").join("d"));
        repo(&root.join("a").join("b").join("c"));
        assert_eq!(find_repos(&root), vec![root.join("a").join("b").join("c")]);
    }
}

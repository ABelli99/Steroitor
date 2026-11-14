//! Modifica del messaggio di un commit non pubblicato, senza rebase interattivo: i commit dal
//! bersaglio a HEAD vengono ricreati con `commit-tree` (stessi tree e autori) e il branch spostato.

use std::collections::HashMap;
use std::path::Path;

use super::runner::Git;

struct RawCommit {
    parents: Vec<String>,
    author: (String, String, String),
    message: String,
}

/// `git cat-file commit`: header fino alla prima riga vuota, poi il messaggio esatto.
fn parse_raw_commit(raw: &str) -> Option<RawCommit> {
    let (headers, message) = raw.split_once("\n\n").unwrap_or((raw, ""));
    let mut parents = Vec::new();
    let mut author = None;
    for line in headers.lines() {
        match line.split_once(' ') {
            Some(("parent", hash)) => parents.push(hash.to_owned()),
            Some(("author", value)) => {
                let (name, rest) = value.split_once(" <")?;
                let (email, date) = rest.split_once("> ")?;
                author = Some((name.to_owned(), email.to_owned(), date.to_owned()));
            }
            _ => {}
        }
    }
    Some(RawCommit { parents, author: author?, message: message.to_owned() })
}

impl Git {
    pub fn reword(&self, root: &str, hash: &str, message: &str) -> Result<(), String> {
        let cwd = Path::new(root);
        if message.trim().is_empty() {
            return Err("Il messaggio di commit è vuoto".into());
        }
        if self.operation(root)?.is_some() {
            return Err("C'è un'operazione in corso (merge, rebase…): concludila prima".into());
        }

        let rev = |spec: &str| self.text(cwd, &["rev-parse", "--verify", "--quiet", spec], true).map(|out| out.trim().to_owned());
        let target = rev(&format!("{hash}^{{commit}}"))?;
        let head = rev("HEAD")?;
        if !self.run(cwd, &["merge-base", "--is-ancestor", &target, "HEAD"], true)?.success {
            return Err("Il commit non fa parte della storia del branch corrente".into());
        }
        if !self.text(cwd, &["branch", "-r", "--contains", &target], true)?.trim().is_empty() {
            return Err("Il commit è già su un branch remoto: cambiarne il messaggio riscriverebbe storia pubblicata".into());
        }

        let descendants = self.text(cwd, &["rev-list", "--reverse", "--topo-order", "--ancestry-path", &format!("{target}..HEAD")], true)?;
        let order = std::iter::once(target.clone()).chain(descendants.lines().map(str::to_owned));

        let mut rewritten: HashMap<String, String> = HashMap::new();
        for commit in order {
            let raw = parse_raw_commit(&self.text(cwd, &["cat-file", "commit", &commit], true)?).ok_or("Commit illeggibile")?;
            let text = if commit == target { format!("{}\n", message.trim_end()) } else { raw.message };
            let tree = format!("{commit}^{{tree}}");
            let parents: Vec<String> = raw.parents.iter().map(|parent| rewritten.get(parent).unwrap_or(parent).clone()).collect();

            let mut args = vec!["commit-tree", tree.as_str()];
            for parent in &parents {
                args.extend(["-p", parent.as_str()]);
            }
            args.extend(["-F", "-"]);
            let (name, email, date) = &raw.author;
            let env = [("GIT_AUTHOR_NAME", name.as_str()), ("GIT_AUTHOR_EMAIL", email.as_str()), ("GIT_AUTHOR_DATE", date.as_str())];
            let created = self.run_full(cwd, &args, Some(text.as_bytes()), &env, false)?;
            if !created.success {
                return Err(created.stderr.trim().to_owned());
            }
            rewritten.insert(commit, created.text().trim().to_owned());
        }

        let reference = self.run(cwd, &["symbolic-ref", "-q", "HEAD"], true)?;
        let reference = if reference.success { reference.text().trim().to_owned() } else { "HEAD".to_owned() };
        let new_head = rewritten.get(&head).ok_or("HEAD non riscritto")?;
        self.text(cwd, &["update-ref", "-m", "steroitor: reword", &reference, new_head, &head], false).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{commit_file, configure, git, Sandbox};
    use super::*;

    fn subjects(git: &Git, root: &str) -> Vec<String> {
        git.text(Path::new(root), &["log", "--format=%s|%an"], false).unwrap().lines().map(str::to_owned).collect()
    }

    fn tree_of(git: &Git, root: &str) -> String {
        git.text(Path::new(root), &["rev-parse", "HEAD^{tree}"], false).unwrap()
    }

    #[test]
    fn parses_raw_commit_headers() {
        let raw = "tree abc\nparent p1\nparent p2\nauthor Ada Lovelace <ada@x.it> 1700000000 +0200\ncommitter C <c@x.it> 1 +0000\n\nTitolo\n\nCorpo\n";
        let commit = parse_raw_commit(raw).unwrap();
        assert_eq!(commit.parents, vec!["p1", "p2"]);
        assert_eq!(commit.author, ("Ada Lovelace".into(), "ada@x.it".into(), "1700000000 +0200".into()));
        assert_eq!(commit.message, "Titolo\n\nCorpo\n");
    }

    #[test]
    fn rewords_an_older_commit_keeping_trees_and_authors() {
        let sandbox = Sandbox::new("reword");
        let root = sandbox.repo("repo");
        let git = git();
        commit_file(&git, &root, "a.txt", "a\n", "primo");
        configure(Path::new(&root), "Bruno", "bruno@example.com");
        commit_file(&git, &root, "b.txt", "b\n", "secondo sbagliato");
        commit_file(&git, &root, "c.txt", "c\n", "terzo");
        let tree = tree_of(&git, &root);
        let target = git.text(Path::new(&root), &["rev-parse", "HEAD~1"], false).unwrap();

        git.reword(&root, target.trim(), "secondo giusto").unwrap();

        assert_eq!(subjects(&git, &root), vec!["terzo|Bruno", "secondo giusto|Bruno", "primo|Test"]);
        assert_eq!(tree_of(&git, &root), tree);
        assert!(git.status(&root).unwrap().files.is_empty());
        assert_eq!(git.status(&root).unwrap().branch.head.as_deref(), Some("main"));
    }

    #[test]
    fn rewords_the_root_commit_through_a_merge() {
        let sandbox = Sandbox::new("reword-merge");
        let root = sandbox.repo("repo");
        let git = git();
        commit_file(&git, &root, "a.txt", "a\n", "radice");
        git.create_branch(&root, "feature", true, None).unwrap();
        commit_file(&git, &root, "f.txt", "f\n", "feature");
        git.switch(&root, "main", false).unwrap();
        commit_file(&git, &root, "m.txt", "m\n", "main");
        git.merge(&root, "feature").unwrap();
        let root_commit = git.text(Path::new(&root), &["rev-list", "--max-parents=0", "HEAD"], false).unwrap();

        git.reword(&root, root_commit.trim(), "radice rinominata").unwrap();

        let log = git.text(Path::new(&root), &["log", "--format=%s", "--topo-order"], false).unwrap();
        assert_eq!(log.lines().last(), Some("radice rinominata"));
        assert_eq!(log.lines().count(), 4, "merge, due commit e radice: nessun duplicato");
        let parents = git.text(Path::new(&root), &["rev-list", "--parents", "-n", "1", "HEAD"], false).unwrap();
        assert_eq!(parents.split_whitespace().count(), 3, "il merge conserva due parent");
    }

    #[test]
    fn refuses_to_reword_pushed_commits() {
        let sandbox = Sandbox::new("reword-pushed");
        let remote = sandbox.bare("remote.git");
        let root = sandbox.repo("repo");
        let git = git();
        git.text(Path::new(&root), &["remote", "add", "origin", &remote], false).unwrap();
        commit_file(&git, &root, "a.txt", "a\n", "pubblicato");
        git.push(&root).unwrap();
        let error = git.reword(&root, "HEAD", "nuovo").unwrap_err();
        assert!(error.contains("remoto"), "{error}");
    }
}

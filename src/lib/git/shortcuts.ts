import type { Handlers } from "../shortcuts";
import { layout } from "../ui/layout.svelte";
import { showPanel } from "../ui/panels";
import type { Workspace } from "../workspace/workspace.svelte";
import { submitCommit, type CommitDraft } from "./commitDraft.svelte";
import type { GitRepo } from "./repo.svelte";
import { showSelectedDiff, type DiffState, type GitSelection } from "./selection.svelte";

const COMMIT_MESSAGE = "#commit-message";

interface Options {
  repo: () => GitRepo | null;
  /** Repository a cui appartiene un file: con più repository non è per forza quello selezionato. */
  repoFor: (path: string) => GitRepo | null;
  workspace: Workspace;
  commitDraft: CommitDraft;
  selection: GitSelection;
  diff: DiffState;
  openVcsMenu: () => void;
  openTerminal: () => void;
}

/** Scorciatoie Git (vedi CLAUDE.md). Ctrl+D e Ctrl+T cambiano azione in base al focus. */
export function gitShortcuts({ repo, repoFor, workspace, commitDraft, selection, diff, openVcsMenu, openTerminal }: Options): Record<string, Handlers> {
  const both = (run: () => void): Handlers => ({ editor: run, git: run });
  const everywhere = (run: () => void): Handlers => ({ editor: run, git: run, terminal: run });
  const withRepo = (run: (repo: GitRepo) => void) => () => {
    const current = repo();
    if (current) run(current);
  };

  const commitShortcut = (andPush: boolean) =>
    withRepo(async (current) => {
      const typing = document.activeElement?.matches(COMMIT_MESSAGE);
      if (!typing || !commitDraft.message.trim() || current.operation) return showPanel("commit", COMMIT_MESSAGE);
      await submitCommit(current, commitDraft, andPush);
    });

  const stageActiveFile = () => {
    const path = workspace.active?.path;
    if (path) repoFor(path)?.stage([path]);
  };

  const stageSelectedFile = withRepo((current) => {
    if (layout.panelTab === "commit" && selection.file) current.stage([selection.file.path]);
  });

  return {
    "Ctrl+K": both(commitShortcut(false)),
    "Ctrl+Alt+K": both(commitShortcut(true)),
    "Ctrl+Shift+K": both(withRepo((current) => current.push())),
    "Ctrl+Alt+A": { editor: stageActiveFile, git: stageSelectedFile },
    "Ctrl+T": { editor: openTerminal, terminal: openTerminal, git: withRepo((current) => current.update()) },
    "Ctrl+D": { git: withRepo((current) => showSelectedDiff(current.root, selection, diff, layout.panelTab === "commit" ? "commit" : "git")) },
    "Alt+9": everywhere(() => showPanel("git")),
    "Alt+`": everywhere(withRepo(() => openVcsMenu())),
  };
}

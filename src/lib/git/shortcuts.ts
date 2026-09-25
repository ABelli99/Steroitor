import type { Handlers } from "../shortcuts";
import { layout } from "../ui/layout.svelte";
import { showPanel } from "../ui/panels";
import { isInsideDir } from "../workspace/files";
import type { Workspace } from "../workspace/workspace.svelte";
import { commitDraft, submitCommit } from "./commitDraft.svelte";
import type { GitRepo } from "./repo.svelte";
import { gitSelection, showSelectedDiff } from "./selection.svelte";

const COMMIT_MESSAGE = "#commit-message";

interface Options {
  repo: () => GitRepo | null;
  workspace: Workspace;
  openVcsMenu: () => void;
  openTerminal: () => void;
}

/** Scorciatoie Git (vedi CLAUDE.md). Ctrl+D e Ctrl+T cambiano azione in base al focus. */
export function gitShortcuts({ repo, workspace, openVcsMenu, openTerminal }: Options): Record<string, Handlers> {
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
      await submitCommit(current, andPush);
    });

  const stageActiveFile = withRepo((current) => {
    const path = workspace.active?.path;
    if (path && isInsideDir(path, current.root)) current.stage([path]);
  });

  const stageSelectedFile = withRepo((current) => {
    if (layout.panelTab === "commit" && gitSelection.file) current.stage([gitSelection.file.path]);
  });

  return {
    "Ctrl+K": both(commitShortcut(false)),
    "Ctrl+Alt+K": both(commitShortcut(true)),
    "Ctrl+Shift+K": both(withRepo((current) => current.push())),
    "Ctrl+Alt+A": { editor: stageActiveFile, git: stageSelectedFile },
    "Ctrl+T": { editor: openTerminal, terminal: openTerminal, git: withRepo((current) => current.update()) },
    "Ctrl+D": { git: () => showSelectedDiff(layout.panelTab === "commit" ? "commit" : "git") },
    "Alt+9": everywhere(() => showPanel("git")),
    "Alt+`": everywhere(withRepo(() => openVcsMenu())),
  };
}

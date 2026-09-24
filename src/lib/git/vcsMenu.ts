import { saveSettings, settings, type UpdateMethod } from "../settings.svelte";
import type { MenuItem } from "../ui/ContextMenu.svelte";
import { OPERATION_LABELS, type Branch } from "./api";
import { relativeTime } from "./format";
import type { GitRepo } from "./repo.svelte";

interface VcsMenuActions {
  openCommit: () => void;
  /** Sostituisce il contenuto del popup (sottomenu). */
  open: (items: MenuItem[]) => void;
}

const METHOD_LABELS: Record<UpdateMethod, string> = { merge: "Merge", rebase: "Rebase" };

const section = (label: string): MenuItem => ({ label, disabled: true, separatorBefore: true, run: () => {} });

function operationItems(repo: GitRepo): MenuItem[] {
  if (!repo.operation) return [];
  const label = OPERATION_LABELS[repo.operation];
  const conflicts = repo.conflicts ? ` · ${repo.conflicts} conflitti` : "";
  return [
    { label: `${label} in corso${conflicts}`, disabled: true, run: () => {} },
    { label: `Continua ${label.toLowerCase()}`, run: () => repo.history.continueOperation() },
    { label: `Annulla ${label.toLowerCase()}…`, danger: true, run: () => repo.history.abortOperation() },
  ];
}

function branchItems(repo: GitRepo, branch: Branch): MenuItem[] {
  const current = repo.branch?.head ?? "HEAD";
  return [
    { label: branch.name, disabled: true, run: () => {} },
    { label: "Checkout", run: () => repo.checkout(branch) },
    { label: `Merge in ${current}`, run: () => repo.history.merge(branch.name) },
    { label: `Rebase ${current} su ${branch.name}`, run: () => repo.history.rebase(branch.name) },
    { label: "Nuovo branch da qui…", run: () => repo.history.branchFrom(branch.name) },
  ];
}

function stashItems(repo: GitRepo, actions: VcsMenuActions): MenuItem[] {
  const items: MenuItem[] = [];
  repo.stashes.forEach((stash, index) => {
    items.push(
      { label: stash.message, hint: relativeTime(stash.timestamp), disabled: true, separatorBefore: index > 0, run: () => {} },
      { label: "Pop (applica e rimuovi)", run: () => repo.history.unstash(stash) },
      { label: "Elimina…", danger: true, run: () => repo.history.dropStash(stash) },
    );
  });
  if (!items.length) items.push({ label: "Nessuno stash", disabled: true, run: () => {} });
  items.push({ label: "← Indietro", separatorBefore: true, run: () => actions.open(vcsMenuItems(repo, actions)) });
  return items;
}

/** Popup del VCS widget: operazioni, metodo di update, stash e branch (con sottomenu). */
export function vcsMenuItems(repo: GitRepo, actions: VcsMenuActions): MenuItem[] {
  const local = repo.branches.filter((branch) => !branch.remote);
  const localNames = new Set(local.map((branch) => branch.name));
  const remote = repo.branches.filter((branch) => branch.remote && !localNames.has(branch.name.slice(branch.name.indexOf("/") + 1)));
  const submenu = (items: MenuItem[]) => () => actions.open(items);

  const items: MenuItem[] = [
    ...operationItems(repo),
    { label: "Update", hint: `Ctrl+T · ${METHOD_LABELS[settings.updateMethod]}`, separatorBefore: !!repo.operation, run: () => repo.update() },
    { label: "Commit…", hint: "Ctrl+K", run: actions.openCommit },
    { label: "Push…", hint: "Ctrl+Shift+K", run: () => repo.push() },
    { label: "Fetch", run: () => repo.fetch() },
    { label: "Nuovo branch…", run: () => repo.newBranch(), separatorBefore: true },
    { label: "Stash modifiche…", run: () => repo.history.stash() },
    { label: `Stash salvati (${repo.stashes.length}) ›`, run: submenu(stashItems(repo, actions)) },
    ...(["merge", "rebase"] as const).map((method, index) => ({
      label: `${settings.updateMethod === method ? "● " : "  "}Update con ${METHOD_LABELS[method]}`,
      separatorBefore: index === 0,
      run: () => {
        settings.updateMethod = method;
        saveSettings();
      },
    })),
  ];

  if (local.length) items.push(section("Branch locali"));
  for (const branch of local) {
    const label = `${branch.current ? "● " : ""}${branch.name}`;
    items.push(branch.current ? { label, hint: "corrente", run: () => {} } : { label: `${label} ›`, run: submenu(branchItems(repo, branch)) });
  }
  if (remote.length) items.push(section("Branch remoti"));
  for (const branch of remote) items.push({ label: `${branch.name} ›`, run: submenu(branchItems(repo, branch)) });
  return items;
}

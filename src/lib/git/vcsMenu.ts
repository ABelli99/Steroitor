import { saveSettings, settings, type UpdateMethod } from "../settings.svelte";
import type { MenuItem } from "../ui/ContextMenu.svelte";
import type { GitRepo } from "./repo.svelte";

interface VcsMenuActions {
  openCommit: () => void;
}

const METHOD_LABELS: Record<UpdateMethod, string> = { merge: "Merge", rebase: "Rebase" };

/** Popup del VCS widget: operazioni, metodo di update e checkout dei branch. */
export function vcsMenuItems(repo: GitRepo, actions: VcsMenuActions): MenuItem[] {
  const local = repo.branches.filter((branch) => !branch.remote);
  const remote = repo.branches.filter((branch) => branch.remote);
  const localNames = new Set(local.map((branch) => branch.name));

  const items: MenuItem[] = [
    { label: "Update", hint: `Ctrl+T · ${METHOD_LABELS[settings.updateMethod]}`, run: () => repo.update() },
    { label: "Commit…", hint: "Ctrl+K", run: actions.openCommit },
    { label: "Push…", hint: "Ctrl+Shift+K", run: () => repo.push() },
    { label: "Fetch", run: () => repo.fetch() },
    { label: "Nuovo branch…", run: () => repo.newBranch(), separatorBefore: true },
    ...(["merge", "rebase"] as const).map((method, index) => ({
      label: `${settings.updateMethod === method ? "● " : "  "}Update con ${METHOD_LABELS[method]}`,
      separatorBefore: index === 0,
      run: () => {
        settings.updateMethod = method;
        saveSettings();
      },
    })),
  ];

  if (local.length) {
    items.push({ label: "Branch locali", disabled: true, separatorBefore: true, run: () => {} });
    for (const branch of local) {
      items.push({
        label: `${branch.current ? "● " : ""}${branch.name}`,
        hint: branch.current ? "corrente" : undefined,
        run: () => repo.checkout(branch),
      });
    }
  }

  const checkoutable = remote.filter((branch) => !localNames.has(branch.name.slice(branch.name.indexOf("/") + 1)));
  if (checkoutable.length) {
    items.push({ label: "Branch remoti", disabled: true, separatorBefore: true, run: () => {} });
    for (const branch of checkoutable) items.push({ label: branch.name, run: () => repo.checkout(branch) });
  }
  return items;
}

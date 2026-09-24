import { invoke } from "@tauri-apps/api/core";

/** Nome della cartella di destinazione, come fa `git clone`: ultimo segmento senza `.git`. */
export function repoNameFromUrl(url: string): string {
  const trimmed = url.trim().replace(/[\\/]+$/, "").replace(/\.git$/i, "");
  const name = trimmed.split(/[\\/:]/).pop() ?? "";
  return name || "repository";
}

export const gitClone = (url: string, target: string) => invoke<void>("git_clone", { url, target });

export const gitInit = (path: string) => invoke<void>("git_init", { path });

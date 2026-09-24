import { invoke } from "@tauri-apps/api/core";
import type { TextFile } from "../backend";

export type FileStatus = "added" | "modified" | "deleted" | "conflict" | "untracked" | "ignored";

export interface BranchInfo {
  head: string | null;
  oid: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
}

export interface StatusEntry {
  path: string;
  status: FileStatus;
  staged: boolean;
  unstaged: boolean;
}

export interface RepoStatus {
  branch: BranchInfo;
  files: StatusEntry[];
}

export interface Branch {
  name: string;
  remote: boolean;
  current: boolean;
  upstream: string | null;
  hash: string;
}

export interface Commit {
  hash: string;
  shortHash: string;
  author: string;
  email: string;
  timestamp: number;
  parents: string[];
  refs: string[];
  subject: string;
  body: string;
}

export const gitStatus = (root: string) => invoke<RepoStatus>("git_status", { root });

export const gitBranches = (root: string) => invoke<Branch[]>("git_branches", { root });

export const gitLog = (root: string, reference: string | null, skip: number, limit: number) =>
  invoke<Commit[]>("git_log", { root, reference, skip, limit });

export const gitHeadContent = (root: string, path: string) => invoke<TextFile | null>("git_head_content", { root, path });

export const gitShowCommit = (root: string, hash: string) => invoke<string>("git_show_commit", { root, hash });

export const gitStage = (root: string, paths: string[]) => invoke<void>("git_stage", { root, paths });

export const gitUnstage = (root: string, paths: string[]) => invoke<void>("git_unstage", { root, paths });

export const gitCommit = (root: string, message: string) => invoke<void>("git_commit", { root, message });

export const gitPush = (root: string) => invoke<void>("git_push", { root });

export const gitPull = (root: string, rebase: boolean) => invoke<void>("git_pull", { root, rebase });

export const gitFetch = (root: string) => invoke<void>("git_fetch", { root });

export const gitSwitch = (root: string, branch: string, remote: boolean) => invoke<void>("git_switch", { root, branch, remote });

export const gitCreateBranch = (root: string, name: string, checkout: boolean) =>
  invoke<void>("git_create_branch", { root, name, checkout });

export const gitStagedCrlfFiles = (root: string) => invoke<string[]>("git_staged_crlf_files", { root });

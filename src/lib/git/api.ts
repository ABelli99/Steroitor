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

export type Operation = "merge" | "rebase" | "cherry-pick" | "revert";

export interface RepoStatus {
  branch: BranchInfo;
  files: StatusEntry[];
  operation: Operation | null;
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

export interface LogFilter {
  reference?: string | null;
  author?: string;
  text?: string;
  path?: string;
}

export const gitLog = (root: string, filter: LogFilter, skip: number, limit: number) =>
  invoke<Commit[]>("git_log", { root, filter, skip, limit });

export const gitHeadContent = (root: string, path: string) => invoke<TextFile | null>("git_head_content", { root, path });

export const gitShowCommit = (root: string, hash: string) => invoke<string>("git_show_commit", { root, hash });

export const gitStage = (root: string, paths: string[]) => invoke<void>("git_stage", { root, paths });

export const gitUnstage = (root: string, paths: string[]) => invoke<void>("git_unstage", { root, paths });

export const gitCommit = (root: string, message: string) => invoke<void>("git_commit", { root, message });

export const gitPush = (root: string) => invoke<void>("git_push", { root });

export const gitPull = (root: string, rebase: boolean) => invoke<void>("git_pull", { root, rebase });

export const gitFetch = (root: string) => invoke<void>("git_fetch", { root });

export const gitSwitch = (root: string, branch: string, remote: boolean) => invoke<void>("git_switch", { root, branch, remote });

export const gitCreateBranch = (root: string, name: string, checkout: boolean, start: string | null = null) =>
  invoke<void>("git_create_branch", { root, name, checkout, start });

export const gitStagedCrlfFiles = (root: string) => invoke<string[]>("git_staged_crlf_files", { root });

export type ResetMode = "soft" | "mixed" | "hard";

export interface BlameLine {
  hash: string;
  author: string;
  timestamp: number;
  summary: string;
}

export interface Stash {
  name: string;
  timestamp: number;
  message: string;
}

export const gitContinueOperation = (root: string) => invoke<void>("git_continue_operation", { root });

export const gitAbortOperation = (root: string) => invoke<void>("git_abort_operation", { root });

export const gitCheckoutCommit = (root: string, hash: string) => invoke<void>("git_checkout_commit", { root, hash });

export const gitCherryPick = (root: string, hash: string) => invoke<void>("git_cherry_pick", { root, hash });

export const gitRevert = (root: string, hash: string) => invoke<void>("git_revert", { root, hash });

export const gitReset = (root: string, hash: string, mode: ResetMode) => invoke<void>("git_reset", { root, hash, mode });

export const gitCreateTag = (root: string, name: string, hash: string) => invoke<void>("git_create_tag", { root, name, hash });

export const gitMerge = (root: string, branch: string) => invoke<void>("git_merge", { root, branch });

export const gitRebase = (root: string, onto: string) => invoke<void>("git_rebase", { root, onto });

export const gitBlame = (root: string, path: string) => invoke<BlameLine[]>("git_blame", { root, path });

export const gitLastCommitMessage = (root: string) => invoke<string>("git_last_commit_message", { root });

export const gitHeadIsPushed = (root: string) => invoke<boolean>("git_head_is_pushed", { root });

export const gitAmend = (root: string, message: string) => invoke<void>("git_amend", { root, message });

export const gitStashes = (root: string) => invoke<Stash[]>("git_stashes", { root });

export const gitStashPush = (root: string, message: string) => invoke<void>("git_stash_push", { root, message });

export const gitStashPop = (root: string, name: string) => invoke<void>("git_stash_pop", { root, name });

export const gitStashDrop = (root: string, name: string) => invoke<void>("git_stash_drop", { root, name });

export const OPERATION_LABELS: Record<Operation, string> = {
  merge: "Merge",
  rebase: "Rebase",
  "cherry-pick": "Cherry-pick",
  revert: "Revert",
};

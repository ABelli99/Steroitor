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

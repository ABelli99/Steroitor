import { invoke } from "@tauri-apps/api/core";

export interface TextFile {
  content: string;
  encoding: string;
  bom: boolean;
}

export const readTextFile = (path: string) => invoke<TextFile>("read_text_file", { path });

export const writeTextFile = (path: string, content: string, encoding: string, bom: boolean) =>
  invoke<void>("write_text_file", { path, content, encoding, bom });

export const loadSession = (folder: string | null) => invoke<string | null>("load_session", { folder });

export const saveSession = (folder: string | null, data: string) => invoke<void>("save_session", { folder, data });

export const startupFiles = () => invoke<string[]>("startup_files");

export const appReady = () => invoke<number>("app_ready");

export interface Entry {
  name: string;
  path: string;
  isDir: boolean;
}

export const listDir = (path: string) => invoke<Entry[]>("list_dir", { path });

export const listFiles = (root: string) => invoke<string[]>("list_files", { root });

export const createFile = (parent: string, name: string) => invoke<string>("create_file", { parent, name });

export const createDir = (parent: string, name: string) => invoke<string>("create_dir", { parent, name });

export const renamePath = (path: string, newName: string) => invoke<string>("rename_path", { path, newName });

export const deleteToTrash = (path: string) => invoke<void>("delete_to_trash", { path });

export const revealInOs = (path: string) => invoke<void>("reveal_in_os", { path });

export const watchFolder = (project: string, path: string | null) => invoke<void>("watch_folder", { project, path });

export const gitRepoRoot = (path: string) => invoke<string | null>("git_repo_root", { path });

export interface ProjectRef {
  id: string;
  folder: string | null;
}

export interface WindowState {
  projects: ProjectRef[];
  active: string | null;
}

export const windowState = () => invoke<WindowState | null>("window_state");

export const setWindowState = (state: WindowState) => invoke<void>("set_window_state", { state });

export const locateProject = (folder: string) => invoke<{ window: string; project: string } | null>("locate_project", { folder });

export const activateProject = (window: string, project: string) => invoke<void>("activate_project", { window, project });

export const detachProject = (project: ProjectRef, x: number, y: number) => invoke<void>("detach_project", { project, x, y });

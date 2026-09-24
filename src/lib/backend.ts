import { invoke } from "@tauri-apps/api/core";

export interface TextFile {
  content: string;
  encoding: string;
  bom: boolean;
}

export const readTextFile = (path: string) => invoke<TextFile>("read_text_file", { path });

export const writeTextFile = (path: string, content: string, encoding: string, bom: boolean) =>
  invoke<void>("write_text_file", { path, content, encoding, bom });

export const loadSession = () => invoke<string | null>("load_session");

export const saveSession = (data: string) => invoke<void>("save_session", { data });

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

export const watchFolder = (path: string | null) => invoke<void>("watch_folder", { path });

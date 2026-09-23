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

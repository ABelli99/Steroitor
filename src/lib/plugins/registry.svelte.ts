import type { Extension } from "@codemirror/state";
import { saveSettings, settings } from "../settings.svelte";
import { javascriptPlugin } from "./javascript";
import type { Plugin, PluginLanguage, PluginModule } from "./types";

export type PluginStatus = "disabled" | "loading" | "ready" | "error";

export const plugins: Plugin[] = [javascriptPlugin];

const isEnabled = (id: string) => !settings.disabledPlugins.includes(id);
const optionsOf = (id: string) => ({ ...settings.pluginOptions[id] });

// I moduli si caricano una volta sola: attivazione e opzioni valgono dal prossimo avvio.
const active = plugins.filter((plugin) => isEnabled(plugin.id));
const startupOptions = Object.fromEntries(plugins.map((plugin) => [plugin.id, optionsOf(plugin.id)]));
const modules = new Map<string, Promise<PluginModule | null>>();
const loaded: PluginModule[] = [];

export const pluginStatus = $state<Record<string, PluginStatus>>(
  Object.fromEntries(plugins.map((plugin) => [plugin.id, active.includes(plugin) ? "loading" : "disabled"])),
);

function load(plugin: Plugin) {
  let module = modules.get(plugin.id);
  if (module) return module;
  module = plugin.load(startupOptions[plugin.id]).then(
    (ready) => {
      loaded.push(ready);
      pluginStatus[plugin.id] = "ready";
      return ready;
    },
    (error) => {
      console.error(`Plugin ${plugin.id}`, error);
      pluginStatus[plugin.id] = "error";
      return null;
    },
  );
  modules.set(plugin.id, module);
  return module;
}

export function startPlugins() {
  for (const plugin of active) void load(plugin);
}

export const pluginEnabled = isEnabled;

export function setPluginEnabled(id: string, enabled: boolean) {
  const others = settings.disabledPlugins.filter((candidate) => candidate !== id);
  settings.disabledPlugins = enabled ? others : [...others, id];
  saveSettings();
}

export const pluginOption = (id: string, option: string) => settings.pluginOptions[id]?.[option] ?? false;

export function setPluginOption(id: string, option: string, enabled: boolean) {
  settings.pluginOptions[id] = { ...settings.pluginOptions[id], [option]: enabled };
  saveSettings();
}

export function needsRestart(plugin: Plugin) {
  if (isEnabled(plugin.id) !== active.includes(plugin)) return true;
  return active.includes(plugin) && (plugin.options ?? []).some((option) => pluginOption(plugin.id, option.id) !== (startupOptions[plugin.id][option.id] ?? false));
}

export function notifyFilesChanged(dirs: string[]) {
  for (const module of loaded) module.onFilesChanged?.(dirs);
}

const activeLanguages = (): PluginLanguage[] => active.flatMap((plugin) => plugin.languages);

export const pluginLanguageFor = (extension: string) => activeLanguages().find((language) => language.fileExtensions.includes(extension)) ?? null;

export const pluginLanguageName = (id: string) => activeLanguages().find((language) => language.id === id)?.name ?? null;

export async function pluginExtensions(language: string): Promise<Extension[]> {
  const modules = await Promise.all(active.map(load));
  return modules.flatMap((module) => module?.extensions[language] ?? []);
}

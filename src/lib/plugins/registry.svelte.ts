import type { Extension } from "@codemirror/state";
import { saveSettings, settings } from "../settings.svelte";
import { javascriptPlugin } from "./javascript";
import type { Plugin, PluginLanguage, PluginModule } from "./types";

export type PluginStatus = "disabled" | "loading" | "ready" | "error";

export const plugins: Plugin[] = [javascriptPlugin];

const isEnabled = (id: string) => !settings.disabledPlugins.includes(id);

// I moduli si caricano una volta sola: attivare o disattivare un plugin vale dal prossimo avvio.
const active = plugins.filter((plugin) => isEnabled(plugin.id));
const modules = new Map<string, Promise<PluginModule | null>>();

export const pluginStatus = $state<Record<string, PluginStatus>>(
  Object.fromEntries(plugins.map((plugin) => [plugin.id, active.includes(plugin) ? "loading" : "disabled"])),
);

function load(plugin: Plugin) {
  let module = modules.get(plugin.id);
  if (module) return module;
  module = plugin.load().then(
    (loaded) => {
      pluginStatus[plugin.id] = "ready";
      return loaded;
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

export const needsRestart = (plugin: Plugin) => isEnabled(plugin.id) !== active.includes(plugin);

const activeLanguages = (): PluginLanguage[] => active.flatMap((plugin) => plugin.languages);

export const pluginLanguageFor = (extension: string) => activeLanguages().find((language) => language.fileExtensions.includes(extension)) ?? null;

export const pluginLanguageName = (id: string) => activeLanguages().find((language) => language.id === id)?.name ?? null;

export async function pluginExtensions(language: string): Promise<Extension[]> {
  const loaded = await Promise.all(active.map(load));
  return loaded.flatMap((module) => module?.extensions[language] ?? []);
}

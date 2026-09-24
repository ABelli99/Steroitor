const STORAGE_KEY = "steroitor.settings";

export type UpdateMethod = "merge" | "rebase";

interface Settings {
  updateMethod: UpdateMethod;
  warnCrlf: boolean;
}

const defaults: Settings = { updateMethod: "merge", warnCrlf: true };

function load(): Settings {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") };
  } catch {
    return defaults;
  }
}

export const settings = $state<Settings>(load());

export function saveSettings() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    /* preferenza non essenziale */
  }
}

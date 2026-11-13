export type SettingsSection = "editor" | "plugins";

export const settingsView = $state<{ open: boolean; section: SettingsSection }>({ open: false, section: "editor" });

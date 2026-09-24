const STORAGE_KEY = "steroitor.layout";

export type PanelTab = "console" | "git" | "commit";

interface LayoutState {
  explorerWidth: number;
  explorerVisible: boolean;
  panelHeight: number;
  panelVisible: boolean;
  panelTab: PanelTab;
}

const defaults: LayoutState = {
  explorerWidth: 240,
  explorerVisible: true,
  panelHeight: 200,
  panelVisible: true,
  panelTab: "console",
};

function load(): LayoutState {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") };
  } catch {
    return defaults;
  }
}

export const layout = $state<LayoutState>(load());

export function saveLayout() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(layout));
  } catch {
    /* preferenza non essenziale */
  }
}

export const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

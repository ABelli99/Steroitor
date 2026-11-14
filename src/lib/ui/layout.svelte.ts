import { readPreference, writePreference } from "../preferences";

export type PanelTab = "console" | "git" | "commit";

interface LayoutState {
  explorerWidth: number;
  explorerVisible: boolean;
  panelHeight: number;
  panelVisible: boolean;
  panelTab: PanelTab;
  commitDetailsWidth: number;
}

const defaults: LayoutState = {
  explorerWidth: 240,
  explorerVisible: true,
  panelHeight: 200,
  panelVisible: true,
  panelTab: "console",
  commitDetailsWidth: 300,
};

export const layout = $state<LayoutState>({ ...defaults, ...readPreference("layout") });

export const saveLayout = () => writePreference("layout", layout);

export const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

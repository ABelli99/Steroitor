import { layout, saveLayout, type PanelTab } from "./layout.svelte";

const FOCUS_ATTEMPTS = 20;

/** Mostra una tab del pannello inferiore e mette il focus su `selector` appena esiste (i pannelli Git sono lazy). */
export function showPanel(tab: PanelTab, selector = "[data-panel-content]") {
  layout.panelVisible = true;
  layout.panelTab = tab;
  saveLayout();

  let attempts = 0;
  const focus = () => {
    const target = document.querySelector<HTMLElement>(selector);
    if (target) return target.focus();
    if (++attempts < FOCUS_ATTEMPTS) requestAnimationFrame(focus);
  };
  requestAnimationFrame(focus);
}

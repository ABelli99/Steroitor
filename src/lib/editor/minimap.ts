import { showMinimap } from "@replit/codemirror-minimap";

/** Caricato solo quando l'utente attiva la minimap. */
export const minimapExtension = showMinimap.of({
  create: () => ({ dom: document.createElement("div") }),
  displayText: "blocks",
  showOverlay: "mouse-over",
});

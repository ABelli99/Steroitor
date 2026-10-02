import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { saveSettings, settings } from "../settings.svelte";
import { LATEST_RELEASE_API, newerRelease, type Update } from "./releases";

/** Dopo l'avvio, per non rubare tempo all'editor che si apre. */
const CHECK_DELAY_MS = 5000;
const TIMEOUT_MS = 5000;

/** Release più recente su GitHub, se è più nuova della versione in uso e non è stata ignorata. */
export const updates = $state<{ available: Update | null }>({ available: null });

/** Una sola richiesta per avvio: nessun controllo periodico in background. Senza rete non succede nulla. */
async function check() {
  const response = await fetch(LATEST_RELEASE_API, {
    headers: { Accept: "application/vnd.github+json" },
    signal: AbortSignal.timeout(TIMEOUT_MS),
  });
  if (!response.ok) return null;
  return newerRelease(await getVersion(), await response.json());
}

export function checkForUpdateLater() {
  setTimeout(async () => {
    const update = await check().catch((error) => {
      console.warn("Controllo aggiornamenti non riuscito:", error);
      return null;
    });
    if (update && update.version !== settings.skippedVersion) updates.available = update;
  }, CHECK_DELAY_MS);
}

export function openUpdate() {
  if (updates.available) invoke("open_release_page", { url: updates.available.url }).catch(console.error);
}

export function skipUpdate() {
  if (!updates.available) return;
  settings.skippedVersion = updates.available.version;
  saveSettings();
  updates.available = null;
}

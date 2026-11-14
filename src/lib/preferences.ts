import { loadPreference, savePreference } from "./backend";

/** Preferenze salvate su file da Rust; il localStorage resta solo come origine per chi aggiorna. */
type Name = "settings" | "layout";

const NAMES: Name[] = ["settings", "layout"];
const LEGACY_KEYS: Record<Name, string> = { settings: "steroitor.settings", layout: "steroitor.layout" };

const loaded = new Map<Name, unknown>();
const writes = new Map<Name, Promise<void>>();

/** Da chiamare prima di montare l'app: i moduli con le preferenze le leggono in modo sincrono all'import. */
export async function loadPreferences() {
  await Promise.all(
    NAMES.map(async (name) => {
      try {
        const text = await loadPreference(name);
        if (text !== null) loaded.set(name, JSON.parse(text));
      } catch (error) {
        console.error(`Preferenze ${name}`, error);
      }
    }),
  );
}

export function readPreference(name: Name): Record<string, unknown> {
  if (loaded.has(name)) return loaded.get(name) as Record<string, unknown>;
  try {
    return JSON.parse(localStorage.getItem(LEGACY_KEYS[name]) ?? "{}");
  } catch {
    return {};
  }
}

/** In fila per nome: una scrittura vecchia non può finire dopo una nuova. */
export function writePreference(name: Name, value: unknown) {
  const data = JSON.stringify(value);
  const previous = writes.get(name) ?? Promise.resolve();
  writes.set(name, previous.then(() => savePreference(name, data)).catch((error) => console.error(`Preferenze ${name}`, error)));
}

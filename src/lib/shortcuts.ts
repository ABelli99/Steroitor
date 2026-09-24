export type ShortcutContext = "editor" | "git";

export type Handlers = Partial<Record<ShortcutContext, () => void>>;

const CONTEXT_ATTRIBUTE = "data-shortcut-context";

/** Il contesto dipende dal focus, non dalla visibilità dei pannelli. */
export function currentContext(): ShortcutContext {
  const scope = document.activeElement?.closest(`[${CONTEXT_ATTRIBUTE}]`);
  return (scope?.getAttribute(CONTEXT_ATTRIBUTE) as ShortcutContext | null) ?? "editor";
}

export function keyOf(event: Pick<KeyboardEvent, "ctrlKey" | "metaKey" | "altKey" | "shiftKey" | "key" | "code">) {
  const parts = [];
  if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  const key = event.code === "Backquote" ? "`" : event.key.length === 1 ? event.key.toUpperCase() : event.key;
  parts.push(key);
  return parts.join("+");
}

export function installShortcuts(bindings: Record<string, Handlers>) {
  const onKeyDown = (event: KeyboardEvent) => {
    const handler = bindings[keyOf(event)]?.[currentContext()];
    if (!handler) return;
    event.preventDefault();
    event.stopPropagation();
    handler();
  };
  window.addEventListener("keydown", onKeyDown, { capture: true });
  return () => window.removeEventListener("keydown", onKeyDown, { capture: true });
}

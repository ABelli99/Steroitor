import type { MenuItem } from "./ContextMenu.svelte";

/** Popup contestuale globale (VCS, gutter, hunk): uno solo alla volta. */
export const menuState = $state<{ current: { x: number; y: number; items: MenuItem[] } | null }>({ current: null });

export function openMenu(x: number, y: number, items: MenuItem[]) {
  if (items.length) menuState.current = { x, y, items };
}

export function closeMenu() {
  menuState.current = null;
}

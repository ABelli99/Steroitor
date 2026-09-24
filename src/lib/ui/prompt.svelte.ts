export interface PromptRequest {
  title: string;
  value: string;
  selectUntil: number;
  multiline: boolean;
  resolve: (value: string | null) => void;
}

export const prompts = $state<{ current: PromptRequest | null }>({ current: null });

export function askText(title: string, value = "", selectUntil = value.length, multiline = false): Promise<string | null> {
  prompts.current?.resolve(null);
  return new Promise((resolve) => {
    prompts.current = { title, value, selectUntil, multiline, resolve };
  });
}

/** Testo su più righe (Ctrl+Invio conferma). */
export const askMultiline = (title: string, value = "") => askText(title, value, value.split("\n")[0].length, true);

export function settlePrompt(value: string | null) {
  const request = prompts.current;
  prompts.current = null;
  request?.resolve(value === null ? null : value.trim());
}

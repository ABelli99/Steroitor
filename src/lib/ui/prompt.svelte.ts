export interface PromptRequest {
  title: string;
  value: string;
  selectUntil: number;
  resolve: (value: string | null) => void;
}

export const prompts = $state<{ current: PromptRequest | null }>({ current: null });

export function askText(title: string, value = "", selectUntil = value.length): Promise<string | null> {
  prompts.current?.resolve(null);
  return new Promise((resolve) => {
    prompts.current = { title, value, selectUntil, resolve };
  });
}

export function settlePrompt(value: string | null) {
  const request = prompts.current;
  prompts.current = null;
  request?.resolve(value?.trim() || null);
}

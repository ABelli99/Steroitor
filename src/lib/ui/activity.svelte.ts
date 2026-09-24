/** Operazione lunga non legata a un repository aperto (es. clone), mostrata nella status bar. */
export const activity = $state<{ label: string | null }>({ label: null });

export async function track<T>(label: string, task: () => Promise<T>): Promise<T> {
  activity.label = label;
  try {
    return await task();
  } finally {
    activity.label = null;
  }
}

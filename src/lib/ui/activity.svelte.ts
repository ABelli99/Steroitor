/** Operazione lunga non legata a un repository aperto (es. clone), mostrata nella status bar. */
export class Activity {
  label = $state<string | null>(null);

  async track<T>(label: string, task: () => Promise<T>): Promise<T> {
    this.label = label;
    try {
      return await task();
    } finally {
      this.label = null;
    }
  }
}

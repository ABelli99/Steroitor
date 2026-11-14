/** Problemi dei plugin da mostrare all'utente (es. un language server che non parte). */
export interface PluginProblem {
  plugin: string;
  message: string;
  /** Dettagli tecnici, mostrati nel tooltip. */
  detail: string;
}

export const pluginProblems = $state<PluginProblem[]>([]);

export function reportPluginProblem(problem: PluginProblem) {
  if (pluginProblems.some((existing) => existing.plugin === problem.plugin && existing.message === problem.message)) return;
  pluginProblems.push(problem);
}

export function dismissPluginProblem(problem: PluginProblem) {
  const index = pluginProblems.indexOf(problem);
  if (index >= 0) pluginProblems.splice(index, 1);
}

import { getContext, setContext } from "svelte";
import type { Project } from "./project.svelte";

const KEY = Symbol("project");

export const provideProject = (project: Project) => setContext(KEY, project);

/** Progetto della vista in cui è montato il componente. */
export const useProject = () => getContext<Project>(KEY);

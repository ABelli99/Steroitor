import { mount } from "svelte";
import "./app.css";
import { loadPreferences } from "./lib/preferences";

// Prima dell'import di App: impostazioni e layout si leggono in modo sincrono quando i loro moduli vengono caricati.
await loadPreferences();
const { default: App } = await import("./App.svelte");

export default mount(App, { target: document.getElementById("app")! });

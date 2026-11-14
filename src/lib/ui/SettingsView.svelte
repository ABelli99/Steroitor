<script lang="ts">
  import { onMount } from "svelte";
  import {
    needsRestart, pluginEnabled, pluginOption, plugins, pluginStatus, setPluginEnabled, setPluginOption, type PluginStatus,
  } from "../plugins/registry.svelte";
  import { pluginProblems } from "../plugins/problems.svelte";
  import { defaultEditorStyle, saveSettings, settings, type EditorStyle } from "../settings.svelte";
  import { settingsView, type SettingsSection } from "./settingsPage.svelte";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();
  let root: HTMLDivElement;

  const sections: { id: SettingsSection; label: string }[] = [
    { id: "editor", label: "Editor" },
    { id: "plugins", label: "Plugin" },
  ];

  const fields: { key: Exclude<keyof EditorStyle, "useTabs">; label: string; min: number; max: number; step: number }[] = [
    { key: "fontSize", label: "Dimensione carattere (px)", min: 8, max: 32, step: 1 },
    { key: "lineHeight", label: "Interlinea", min: 1, max: 2.5, step: 0.1 },
    { key: "tabSize", label: "Larghezza tab", min: 1, max: 16, step: 1 },
    { key: "indentSize", label: "Indentazione (spazi)", min: 1, max: 16, step: 1 },
  ];

  const statusLabel: Record<PluginStatus, string> = { disabled: "Non caricato", loading: "In caricamento", ready: "Caricato", error: "Errore di caricamento" };

  onMount(() => root.focus());

  function setField(field: (typeof fields)[number], raw: string) {
    const value = Number(raw);
    settings.editor[field.key] = Number.isFinite(value) ? Math.min(field.max, Math.max(field.min, value)) : defaultEditorStyle[field.key];
    saveSettings();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();
    onclose();
  }
</script>

<div bind:this={root} class="settings" role="dialog" aria-modal="true" aria-label="Impostazioni" tabindex="-1" data-shortcut-context="settings" onkeydown={onKeyDown}>
  <nav>
    <h1>Impostazioni</h1>
    {#each sections as section (section.id)}
      <button class:on={settingsView.section === section.id} onclick={() => (settingsView.section = section.id)}>{section.label}</button>
    {/each}
  </nav>

  <main>
    <header>
      <h2>{sections.find((section) => section.id === settingsView.section)?.label}</h2>
      <button title="Chiudi (Esc)" aria-label="Chiudi" onclick={onclose}>✕</button>
    </header>

    {#if settingsView.section === "editor"}
      <section class="form">
        {#each fields as field (field.key)}
          <label>
            <span>{field.label}</span>
            <input
              type="number"
              min={field.min}
              max={field.max}
              step={field.step}
              value={settings.editor[field.key]}
              onchange={(event) => setField(field, event.currentTarget.value)}
            />
          </label>
        {/each}
        <label class="check">
          <input type="checkbox" bind:checked={settings.editor.useTabs} onchange={saveSettings} />
          <span>Indenta con il carattere tab</span>
        </label>
      </section>
    {:else}
      <p class="note">I plugin attivi si caricano una volta all'avvio: attivarli o disattivarli vale dal prossimo avvio.</p>
      <ul class="plugins">
        {#each plugins as plugin (plugin.id)}
          <li>
            <div class="head">
              <strong>{plugin.name}</strong>
              <span class="muted">{plugin.version} · {plugin.author}</span>
              <span class="spacer"></span>
              <label class="check">
                <input type="checkbox" checked={pluginEnabled(plugin.id)} onchange={(event) => setPluginEnabled(plugin.id, event.currentTarget.checked)} />
                <span>Attivo</span>
              </label>
            </div>
            <p>{plugin.description}</p>
            <div class="languages">
              {#each plugin.languages as language (language.id)}
                <span class="chip">{language.name} <span class="muted">.{language.fileExtensions.join(" .")}</span></span>
              {/each}
            </div>
            {#each plugin.options ?? [] as option (option.id)}
              <label class="check option">
                <input
                  type="checkbox"
                  checked={pluginOption(plugin.id, option.id)}
                  disabled={!pluginEnabled(plugin.id)}
                  onchange={(event) => setPluginOption(plugin.id, option.id, event.currentTarget.checked)}
                />
                <span>{option.label}</span>
              </label>
              <p class="muted option-text">{option.description}</p>
            {/each}
            <small class:error={pluginStatus[plugin.id] === "error"}>
              {statusLabel[pluginStatus[plugin.id]]}{needsRestart(plugin) ? " · riavvia per applicare" : ""}
            </small>
            {#each pluginProblems.filter((problem) => problem.plugin === plugin.name) as problem (problem.message)}
              <div class="problem">
                <strong>⚠ {problem.message}</strong>
                <pre>{problem.detail}</pre>
              </div>
            {/each}
          </li>
        {/each}
      </ul>
    {/if}
  </main>
</div>

<style>
  .settings {
    position: fixed;
    inset: 0;
    z-index: 15;
    display: flex;
    background: var(--bg);
    outline: none;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 200px;
    flex: none;
    padding: 12px 8px;
    background: var(--panel-bg);
    border-right: 1px solid var(--border);
  }

  h1 {
    margin: 0 8px 12px;
    font-size: 15px;
    font-weight: 600;
  }

  nav button {
    text-align: left;
    padding: 6px 8px;
  }

  nav button.on {
    color: var(--accent);
    background: var(--hover);
  }

  main {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 12px 24px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 420px;
  }

  label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  label.check {
    justify-content: flex-start;
    gap: 6px;
  }

  input[type="number"] {
    width: 80px;
    font: inherit;
    color: var(--fg);
    background: var(--editor-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 3px 6px;
  }

  input[type="number"]:focus {
    outline: none;
    border-color: var(--accent);
  }

  .note,
  .muted {
    color: var(--fg-muted);
  }

  .note {
    margin: 0 0 12px;
  }

  .plugins {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 720px;
  }

  .plugins li {
    padding: 10px 12px;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .spacer {
    flex: 1;
  }

  .plugins p {
    margin: 6px 0;
  }

  .languages {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }

  .plugins p.option-text {
    margin: 2px 0 8px 22px;
  }

  .chip {
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 10px;
    font-size: 12px;
  }

  small {
    color: var(--fg-muted);
  }

  small.error,
  .problem strong {
    color: var(--danger);
  }

  .problem {
    margin-top: 8px;
  }

  .problem pre {
    max-height: 160px;
    overflow: auto;
    margin: 4px 0 0;
    padding: 6px 8px;
    font: 12px var(--mono);
    white-space: pre-wrap;
    color: var(--fg-muted);
    background: var(--editor-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
  }
</style>

<script lang="ts">
  import { onMount } from "svelte";
  import { history, historyKeymap, defaultKeymap } from "@codemirror/commands";
  import { EditorState, Text, type Extension } from "@codemirror/state";
  import { EditorView, keymap, lineNumbers } from "@codemirror/view";
  import { ask, message } from "@tauri-apps/plugin-dialog";
  import { readTextFile, writeTextFile } from "../backend";
  import { detectLanguage, loadLanguage } from "../editor/languages";
  import { editorTheme } from "../editor/theme";
  import { detectEol, fileName } from "../workspace/files";
  import { gitConflictVersions } from "./api";
  import { conflictEditor, jumpToConflict } from "./conflictEditor";
  import { parseConflicts, resolveAll } from "./conflicts";
  import { gitGutter } from "./gutter";
  import type { GitRepo } from "./repo.svelte";

  let { repo, path, onclose }: { repo: GitRepo; path: string; onclose: () => void } = $props();

  let oursHost = $state<HTMLElement>();
  let resultHost = $state<HTMLElement>();
  let theirsHost = $state<HTMLElement>();
  let remaining = $state(0);
  let error = $state<string | null>(null);
  let saving = $state(false);

  let result: EditorView | null = null;
  let initial: Text | null = null;
  let file = { encoding: "UTF-8", bom: false, eol: "\n" as "\n" | "\r\n" };

  const toText = (content: string) => Text.of(content.split(/\r\n|\r|\n/));

  function readOnly(parent: HTMLElement, doc: string, base: Text, language: Extension) {
    return new EditorView({
      parent,
      state: EditorState.create({
        doc,
        extensions: [lineNumbers(), gitGutter(base), EditorState.readOnly.of(true), EditorView.editable.of(false), editorTheme, language],
      }),
    });
  }

  onMount(() => {
    const views: EditorView[] = [];
    Promise.all([gitConflictVersions(repo.root, path), readTextFile(path), loadLanguage(detectLanguage(path))])
      .then(([versions, current, language]) => {
        file = { encoding: current.encoding, bom: current.bom, eol: detectEol(current.content, "\n") };
        const base = toText(versions.base ?? "");
        views.push(readOnly(oursHost!, versions.ours ?? "", base, language));
        views.push(readOnly(theirsHost!, versions.theirs ?? "", base, language));
        result = new EditorView({
          parent: resultHost!,
          state: EditorState.create({
            doc: current.content,
            extensions: [
              lineNumbers(),
              history(),
              keymap.of([
                { key: "F7", run: (view) => jumpToConflict(view, 1) },
                { key: "Shift-F7", run: (view) => jumpToConflict(view, -1) },
                ...defaultKeymap,
                ...historyKeymap,
              ]),
              conflictEditor,
              editorTheme,
              language,
              EditorView.updateListener.of((update) => {
                if (update.docChanged) remaining = parseConflicts(update.state.doc.toString()).length;
              }),
            ],
          }),
        });
        initial = result.state.doc;
        views.push(result);
        remaining = parseConflicts(current.content).length;
        jumpToConflict(result, 1);
        result.focus();
      })
      .catch((failure) => (error = String(failure)));
    return () => views.forEach((view) => view.destroy());
  });

  function acceptAll(choice: "ours" | "theirs") {
    if (!result) return;
    const text = result.state.doc.toString();
    result.dispatch({ changes: { from: 0, to: text.length, insert: resolveAll(text, choice) } });
  }

  async function apply() {
    if (!result || saving) return;
    if (remaining && !(await ask(`Restano ${remaining} conflitti non risolti. Salvare comunque e segnare il file come risolto?`, { title: "Merge", kind: "warning" })))
      return;
    saving = true;
    try {
      await writeTextFile(path, result.state.doc.toJSON().join(file.eol), file.encoding, file.bom);
      await repo.stage([path]);
      await repo.workspace.reloadClean([path]);
      onclose();
    } catch (failure) {
      await message(String(failure), { title: "Salvataggio non riuscito", kind: "error" });
    } finally {
      saving = false;
    }
  }

  async function cancel() {
    const changed = result && initial && !result.state.doc.eq(initial);
    if (changed && !(await ask("Chiudere senza salvare le risoluzioni fatte?", { title: "Merge", kind: "warning" }))) return;
    onclose();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") cancel();
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-label="Risolvi conflitti" tabindex="-1" onkeydown={onKeyDown}>
    <header>
      <span class="title">Conflitti in {fileName(path)}</span>
      <span class="count" class:done={!remaining}>{remaining ? `${remaining} da risolvere` : "tutti risolti"}</span>
      <span class="spacer"></span>
      <button onclick={() => result && jumpToConflict(result, -1)} title="Shift+F7">◀ Precedente</button>
      <button onclick={() => result && jumpToConflict(result, 1)} title="F7">Successivo ▶</button>
      <button onclick={() => acceptAll("ours")}>Tutte nostre</button>
      <button onclick={() => acceptAll("theirs")}>Tutte loro</button>
      <button class="primary" disabled={saving} onclick={apply}>Applica e segna risolto</button>
      <button onclick={cancel}>Annulla</button>
    </header>
    {#if error}<p class="error">{error}</p>{/if}
    <div class="labels"><span>Nostra (HEAD)</span><span>Result</span><span>Loro</span></div>
    <div class="panes">
      <div class="pane" bind:this={oursHost}></div>
      <div class="pane result" bind:this={resultHost}></div>
      <div class="pane" bind:this={theirsHost}></div>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 25;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.3);
  }

  .dialog {
    width: calc(100vw - 48px);
    height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    background: var(--editor-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    overflow: hidden;
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    padding: 6px 10px;
    background: var(--panel-bg);
    border-bottom: 1px solid var(--border);
  }

  header button {
    border: 1px solid var(--border);
    padding: 2px 8px;
  }

  header .primary:not(:disabled) {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .title {
    font-weight: 600;
  }

  .count {
    color: var(--danger);
  }

  .count.done {
    color: var(--git-added);
  }

  .spacer {
    flex: 1;
  }

  .labels,
  .panes {
    display: grid;
    grid-template-columns: 1fr 1.2fr 1fr;
  }

  .labels {
    flex: none;
    padding: 2px 8px;
    font-size: 11px;
    color: var(--fg-muted);
    border-bottom: 1px solid var(--border);
  }

  .panes {
    flex: 1;
    min-height: 0;
  }

  .pane {
    min-width: 0;
    overflow: hidden;
    border-right: 1px solid var(--border);
    user-select: text;
  }

  .pane :global(.cm-editor) {
    height: 100%;
  }

  .result {
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  .error {
    margin: 8px 12px;
    color: var(--danger);
  }
</style>

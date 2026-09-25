<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";
  import { useProject } from "../project/context";

  let { id, visible }: { id: number; visible: boolean } = $props();
  const { terminals } = useProject();

  let host = $state<HTMLElement>();
  let terminal: Terminal | null = null;
  let fit: FitAddon | null = null;

  const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

  function fitNow() {
    if (!terminal || !fit || !host?.clientWidth) return;
    fit.fit();
    terminals.resize(id, terminal.cols, terminal.rows);
  }

  onMount(() => {
    terminal = new Terminal({
      fontFamily: cssVar("--mono"),
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      theme: {
        background: cssVar("--editor-bg"),
        foreground: cssVar("--fg"),
        cursor: cssVar("--accent"),
        selectionBackground: cssVar("--selection"),
      },
    });
    fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(host!);

    const typed = terminal.onData((data) => terminals.write(id, data));
    const detach = terminals.attach(id, (data) => terminal?.write(data));
    const observer = new ResizeObserver(() => fitNow());
    observer.observe(host!);
    fitNow();
    terminal.focus();

    return () => {
      observer.disconnect();
      detach();
      typed.dispose();
      terminal?.dispose();
      terminal = null;
    };
  });

  $effect(() => {
    if (!visible) return;
    requestAnimationFrame(() => {
      fitNow();
      terminal?.focus();
    });
  });
</script>

<div class="terminal" class:hidden={!visible} bind:this={host} data-shortcut-context="terminal"></div>

<style>
  .terminal {
    height: 100%;
    padding: 2px 0 0 6px;
    background: var(--editor-bg);
  }

  .hidden {
    display: none;
  }
</style>

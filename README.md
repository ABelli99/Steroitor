# Steroitor

Un **notepad potenziato** per Windows, con un'integrazione Git modellata su IntelliJ IDEA.
Si apre in mezzo secondo, pesa 2 MB e aggiunge Git solo quanto basta: non è un IDE.

## Perché esiste

Steroitor è un **esercizio**. Volevo capire se Claude, il modello di Anthropic, riesce a scrivere
un'applicazione vera **tenendo conto dei cicli macchina**: non solo codice che funziona, ma codice che
costa poco in CPU, memoria e tempo di avvio. Ogni scelta è passata da tre domande:

1. Serve a chi usa l'app come notepad tutti i giorni?
2. Rispetta i budget di performance?
3. Si può caricare solo quando serve, senza pesare sull'avvio?

Se la risposta a una delle tre era "no", la funzione restava fuori. Il codice è stato scritto da Claude
(Claude Code) seguendo [`CLAUDE.md`](CLAUDE.md), che descrive l'app, e [`ROADMAP.md`](ROADMAP.md), che
fissa budget, fasi e confini. Non è destinato alla distribuzione commerciale.

## Cosa fa

**Editor**
- Tab multiple, ricerca e sostituzione, multi-cursore, word wrap, minimap opzionale
- Evidenziazione per JS/TS, Python, Java, Markdown, JSON, YAML, HTML, CSS
- Rispetta encoding ed EOL (CRLF/LF) dei file
- Salvataggio automatico e ripristino della sessione, compreso il testo non salvato
- Istanza singola e associazione "Apri con"

**Progetti**
- Explorer con albero lazy, Quick Open fuzzy (`Ctrl+P`) e aggiornamento automatico dal disco
- Più progetti nella stessa finestra, come tab nella barra laterale sinistra: si riordinano trascinandoli
  e, trascinati fuori dalla barra, diventano una finestra separata
- Più repository nella stessa cartella (es. `gipso/gipso-fe` e `gipso/gipso-be`), ognuno con branch e
  history propri

**Git** (usa l'eseguibile `git` di sistema; ogni comando finisce nella Console)
- Colori dei file nell'Explorer e marker nel gutter, come in IntelliJ
- Pannello Commit con staging, diff, commit parziale per hunk o riga, amend, e apertura del file sulla
  prima riga modificata (F4)
- Log con grafo, filtri, checkout, cherry-pick, revert, reset, tag, modifica del messaggio di commit
- Push, pull (merge o rebase), fetch, branch, merge, rebase, stash, blame
- Merge tool a 3 vie per i conflitti
- Terminale interattivo (PowerShell, cmd, Git Bash…) accanto al log dei comandi

Le scorciatoie seguono IntelliJ: `Ctrl+K` commit, `Ctrl+Alt+K` commit and push, `Ctrl+Shift+K` push,
`Alt+9` pannello Git, ``Alt+` `` popup VCS. `Ctrl+D` e `Ctrl+T` cambiano significato in base al focus
(vedi la roadmap).

## I budget e come sono andati

I limiti sono vincolanti: una fase non si chiude se ne sfora uno. Si misurano con `npm run measure`
sulla build di release, partendo sempre da una sessione pulita.

| Metrica | Target | Limite | Misurato (1.1.1) |
|---|---|---|---|
| Installer | < 10 MB | 15 MB | **2,1 MB** |
| RAM a riposo, 1 file | < 80 MB | 120 MB | **75,7–79,8 MB** |
| RAM con un repository aperto | – | 120 MB | 79–83 MB |
| Avvio → editor usabile | < 500 ms | 1 s | **522–610 ms** a regime |
| Primo avvio dopo una build | < 500 ms | 1 s | 1,0 s (cache del disco fredda) |

Il risultato, detto senza abbellimenti: dimensione e memoria stanno nel target con margine.
L'avvio sta nel limite ma sopra il target. Il primo avvio dopo una build, a cache fredda, arriva al limite.

Per confronto, un editor Electron vuoto parte da circa 150 MB di RAM e 80 MB di installer.

### Scelte fatte per i cicli macchina

- **Tauri invece di Electron**: usa il WebView2 già presente in Windows, quindi niente browser incluso
  nell'installer.
- **Svelte 5 e CodeMirror 6** invece di React e Monaco: niente runtime pesante, editor modulare.
- **Tutto ciò che non serve all'avvio si carica dopo.** Il modulo Git si carica solo se la cartella è un
  repository. I linguaggi si caricano al primo file di quel tipo; pannelli Git, merge tool, terminale e
  minimap alla prima apertura.
- **WebView2 senza GPU** (`--disable-gpu`): risparmia circa 40 MB di processo GPU, in cambio di un
  rendering software.
- **Log Git virtualizzato e paginato** a 200 commit: su un repository di 329 commit la RAM è scesa da
  circa 130 a 95 MB.
- **Watcher con debounce** (200 ms), che ignora `.git`, `node_modules` e il `.gitignore` di ogni
  repository. Lo status Git si ricalcola con un ulteriore debounce, e solo per i repository toccati.
- **Git senza lock opzionali** (`GIT_OPTIONAL_LOCKS=0`): i controlli in background non bloccano i comandi
  lanciati da terminale.
- **I progetti ripristinati all'avvio non si caricano finché non li apri.**
- **Nessun client HTTP in Rust**: il controllo degli aggiornamenti parte dalla webview. Un client in Rust
  costava +0,5 MB di installer per una sola richiesta.
- **La ricerca dei repository annidati legge solo il filesystem**, senza processi git: tre livelli al
  massimo, saltando `node_modules`, `target` e le cartelle nascoste.

Le decisioni prese contro la roadmap sono scritte lì, con il loro costo: più progetti per finestra, più
repository per progetto, controllo della versione su GitHub.

## Installazione

Scarica `Steroitor_X.Y.Z_x64-setup.exe` dalla pagina
[Releases](https://github.com/ABelli99/Steroitor/releases) e lancialo. L'installazione è per l'utente
corrente e non chiede privilegi di amministratore. Un setup più nuovo aggiorna quella esistente, senza
installarne una seconda. All'avvio Steroitor controlla una volta se c'è una release più nuova e lo
segnala nella status bar.

Richiede Git installato (rilevato dal PATH, oppure `gitPath` in
`%APPDATA%\it.overzoom.steroitor\settings.json`).

## Sviluppo

Servono Node.js 20 o più recente, Rust stabile e Git.

```
npm install
npm run tauri dev       # avvio in sviluppo
npm test                # test frontend (Vitest)
npm run check           # controllo dei tipi (svelte-check)
cd src-tauri && cargo test
npm run measure         # build di release e misura dei budget
```

Struttura:

```
src/                 frontend Svelte
  lib/project/       un progetto: editor, explorer, Git, console, terminali
  lib/git/           pannelli e logica Git
  lib/workspace/     tab, file, sessione
src-tauri/src/       backend Rust: file, watcher, terminali, finestre, comandi git
scripts/measure.ps1  misura dei budget
```

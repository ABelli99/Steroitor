# ROADMAP — Steroitor

> Obiettivo: un **notepad potenziato**, non un IDE. Si apre in un attimo, pesa poco,
> e aggiunge Git solo quanto basta. Il CLAUDE.md descrive il perimetro massimo;
> questa roadmap decide **cosa entra, quando, e cosa resta fuori**.

---

## Regola guida

Ogni feature deve passare questo filtro:

1. Serve a chi usa l'app come notepad quotidiano?
2. Rispetta i budget di performance (sotto)?
3. Si può caricare in lazy, senza pesare sull'avvio?

Se la risposta a una delle tre è "no", va in backlog.

---

## Budget di performance (vincolanti)

| Metrica | Target | Limite |
|---|---|---|
| Installer | < 10 MB | 15 MB |
| RAM a riposo (1 file aperto) | < 80 MB | 120 MB |
| Avvio a freddo → editor usabile | < 500 ms | 1 s |
| Apertura file da 10 MB | < 1 s | 2 s |
| Input latency durante la digitazione | < 16 ms | — |

Si misurano da F0 in poi (`npm run measure`, oppure `-SkipBuild` lanciando lo script direttamente; `-Repo <percorso>` per una misura informativa con un repository aperto).
Lo script mette da parte la sessione dell'utente e parte sempre da uno stato pulito.
Una fase non è chiusa se sfora un limite.

Ultima misura (post-F6, build release, sessione isolata): installer 2,0 MB · RAM 75–79 MB · avvio a regime 515–670 ms.
Con un repository reale aperto (329 commit, tab Git visibile): RAM ~95 MB — prima della virtualizzazione del log era ~130 MB.
**Rischio aperto:** il primo avvio dopo una build (cache file del sistema operativo fredda) arriva a 1,0–1,3 s, oltre il limite; a regime e con profilo WebView2 vuoto resta ~500 ms. Da ricontrollare dopo un riavvio del PC.

**Decisione:** WebView2 gira con `--disable-gpu` (`tauri.conf.json` → `additionalBrowserArgs`).
Risparmia ~40 MB (processo GPU). Costo: rendering software, da tenere d'occhio su
scroll di file grandi e schermi ad alta risoluzione.

---

## Stack

| Area | Scelta | Perché | Scartato |
|---|---|---|---|
| Shell desktop | **Tauri 2** | Usa WebView2 di sistema, binario piccolo, backend Rust | Electron (150 MB+ RAM, 80 MB+ installer) |
| UI | **Svelte 5** + Vite | Nessun runtime pesante, compila a JS minimo | React (runtime + ecosistema più pesante per nessun guadagno qui) |
| Editor | **CodeMirror 6** | Modulare, linguaggi caricabili on-demand, ottimo su file grandi | Monaco (~5 MB, pensato per un IDE vero) |
| Git | **Eseguibile `git` di sistema** via Rust (`std::process::Command`) | Comportamento identico a Git reale, output diretto in Console, SSH/credenziali gratis | libgit2/git2 (gap su SSH/credential helper), isomorphic-git (lento su repo grandi) |
| File watching | crate `notify` | Standard, cross-platform | polling |
| Diff in-editor | `@codemirror/merge` | Stesso motore dell'editor | librerie diff separate |

---

## Fasi

### F0 — Scheletro (fondamenta) ✅

- Progetto Tauri 2 + Svelte 5 + CodeMirror 6
- Layout a tre zone ridimensionabili e collassabili: Explorer | Editor | Pannello inferiore
- Tema chiaro/scuro seguendo il sistema
- Script di misura dei budget (dimensione bundle, RAM, tempo di avvio)

**Uscita:** finestra vuota con layout funzionante, budget misurati e rispettati.

---

### F1 — Notepad core (il prodotto minimo) — implementata, in verifica manuale

Se l'app si fermasse qui, dovrebbe già sostituire Notepad/Notepad++ per l'uso base.

- Apri / Salva / Salva con nome / Nuovo file
- Multi-tab: chiusura, riordino, indicatore dirty, conferma su chiusura non salvata
- Undo/redo, selezione, copia/incolla, multi-cursore (gratis con CM6)
- Numeri di riga, word wrap, ricerca e sostituzione (`Ctrl+F` / `Ctrl+H`)
- Syntax highlighting con linguaggi caricati **on-demand**: JS/TS, Python, Java, Markdown, JSON (+ YAML, HTML, CSS)
- Rilevamento e preservazione di **encoding** ed **EOL** (CRLF/LF) — su Windows è critico
- Status bar: riga/colonna, encoding, EOL, linguaggio
- **Ripristino sessione**: tab aperte e contenuto non salvato sopravvivono al riavvio (come Notepad++)
- Associazione "Apri con" + istanza singola (un secondo avvio apre il file nella finestra esistente)

**Uscita:** uso quotidiano come notepad senza rimpianti, budget rispettati.

---

### F2 — File Explorer — implementata, in verifica manuale

- Apertura cartella → albero con caricamento **lazy** delle sottocartelle
- Click su file → apre o porta in focus la tab
- Context menu: New File, New Folder, Rename, Delete, Reveal in Explorer, Copy Path
- Refresh automatico via file watcher, con esclusione di `.git`, `node_modules` e path in `.gitignore`
- Quick Open (`Ctrl+P`) con ricerca fuzzy sui nomi file

**Uscita:** navigazione fluida su un repo da ~10k file senza impatto su RAM/avvio.

---

### F3 — Git in sola lettura — implementata, in verifica manuale

Tutto il modulo Git si carica **solo** se la cartella aperta contiene `.git`.

- Rilevamento automatico di `git` nel PATH (path configurabile)
- VCS widget in status bar: branch corrente
- Colori file nell'Explorer (verde / blu / grigio-rosso) da `git status --porcelain=v2`
- Marker nel gutter dell'editor (righe aggiunte / modificate / cancellate) contro `HEAD`
- Tab **Console**: log di ogni comando Git eseguito, con output, exit code e durata. Solo output, niente shell
- Tab **Git**: lista branch (locali + remoti) e log con commit id, data, autore, messaggio. Paginato (es. 200 commit alla volta)

**Uscita:** apro un repo e vedo stato, branch e history senza aver scritto una riga di Git.

Note di implementazione:
- Path di git configurabile con `gitPath` in `%APPDATA%/it.overzoom.steroitor/settings.json` (niente UI per ora).
- La Console nasconde di default i comandi in background riusciti (status, show, rev-parse); un checkbox li mostra.
- Cambi a branch/index fatti da fuori (terminale) si vedono solo se `.git` è dentro la cartella aperta.

---

### F4 — Git operativo (il flusso quotidiano) — implementata, in verifica manuale

- Pannello **Commit**: lista file modificati / untracked / cancellati, checkbox, messaggio, Commit / Commit and Push
- Solo **staging area**. Niente changelist (vedi "Fuori scope")
- Diff preview del file selezionato (`@codemirror/merge`)
- Push / Pull / Fetch, con update method Merge (default) o Rebase
- Checkout branch, New Branch dal VCS widget
- Scorciatoie: `Ctrl+K`, `Ctrl+Alt+K`, `Ctrl+Shift+K`, `Ctrl+Alt+A`, `Alt+9`, più `Ctrl+D` / `Ctrl+T` contestuali (vedi "Scorciatoie contestuali")
- Warning su detached HEAD e su CRLF

**Uscita:** modifica → commit → push senza uscire dall'app.

Note di implementazione:
- Checkbox del pannello Commit = staging reale (`git add` / `git restore --staged`); stato misto = checkbox indeterminata.
- `Ctrl+Shift+K` (Push) vince su "elimina riga" di CodeMirror anche nell'editor.
- Push senza upstream: chiede conferma e pubblica su `origin` (o sul primo remote) con `--set-upstream`.
- Metodo di update (Merge/Rebase) e avviso CRLF sono preferenze locali, dal popup VCS (``Alt+` `` o click sul branch).

---

### F5 — History avanzata — implementata, in verifica manuale

- Grafo dei commit nel log (algoritmo a corsie, disegnato in SVG)
- Filtri: branch, autore, path, testo nel messaggio
- Azioni dal log: Checkout commit, Cherry-Pick, Revert, Reset (soft/mixed/hard, con conferma su hard), Create Tag, Copy Commit ID, Show Changes
- Merge / Rebase dal VCS widget, con stato "conflitto" visibile e file in conflitto elencati
- Annotate / Blame (on-demand, da context menu)
- Amend dell'ultimo commit
- Stash (push / pop / list)

**Uscita:** si può gestire la history ordinaria senza aprire un terminale.

Note di implementazione:
- Il grafo si nasconde quando sono attivi filtri su autore/testo/path (i parent non sono più contigui).
- Merge/rebase/cherry-pick/revert fermi su conflitti: stato in status bar, Continua/Annulla nel pannello Commit e nel popup VCS.
- Annotate: click destro sul gutter dell'editor o sul file nell'Explorer; click su un'annotazione apre il commit.
- Amend di un commit già pushato chiede conferma: il push forzato resta da terminale.

---

## F6 — Backlog (dopo la chiusura di F5) — completato

Si parte solo a roadmap completata. Ogni voce resta soggetta ai budget di performance.
In ordine di priorità:

1. ✅ Merge tool a 3 vie (Yours / Result / Theirs) — apribile da "Risolvi…" o doppio click su un file in conflitto; F7 / Shift+F7 tra i conflitti
2. ✅ Terminale interattivo vero (xterm.js + PTY) come seconda sotto-tab della Console — PowerShell su Windows, più istanze, i tasti vanno alla shell tranne Alt+1 / Alt+9 / Alt+`
3. ✅ Commit parziale per hunk / riga, stage/rollback dal gutter — click su un marker: rollback, stage del hunk o delle righe selezionate (l'index si aggiorna senza toccare il file)
4. ✅ Clone da URL e `git init` dall'UI — pulsante Clona (top bar, Explorer vuoto, tab Git) e git init dalla tab Git
5. ✅ Edit Commit Message di commit non pushati — dal menu del log; riscrive i commit con commit-tree (tree e autori invariati), rifiuta i commit già su un remote
6. ✅ Minimap — toggle nella status bar, estensione caricata solo quando attiva, preferenza salvata nella sessione

---

## Fuori scope (decisione esplicita)

Presenti nel CLAUDE.md ma incompatibili con "notepad leggero":

| Feature | Motivo |
|---|---|
| Changelist stile IntelliJ | Duplica la staging area, logica complessa, beneficio basso |
| Interactive Rebase | UI complessa, uso raro; il terminale basta |
| Local History | Richiede storage persistente e indicizzazione continua |
| Multi-repository nello stesso progetto | Moltiplica watcher e stato Git |
| Integrazione GitHub/GitLab (PR, ecc.) | Dipendenze di rete e auth, fuori dal ruolo di un notepad |
| GPG signing, protected branches, SSH built-in | Si delega a Git di sistema e alla sua config |
| Auto-fetch | Traffico e processi in background non richiesti |

**Eccezione decisa (1.1):** più progetti nella stessa finestra, come tab nella barra laterale sinistra, e
trascinabili fuori in una finestra propria. Ogni progetto ha il suo watcher, il suo stato Git e i suoi
terminali, quindi il costo è quello indicato sopra, moltiplicato per i progetti aperti. Per contenerlo, un
progetto ripristinato all'avvio non si carica finché non lo apri. Il budget RAM vale per un progetto solo.

---

## Scorciatoie contestuali

Alcune scorciatoie cambiano significato in base a **dove si trova il focus**:

| Scorciatoia | Focus su tab Git / Commit | Focus sull'editor (o altrove) |
|---|---|---|
| `Ctrl+D` | Show Diff del commit/file selezionato | Seleziona occorrenza successiva |
| `Ctrl+T` | Update / Pull | Nuovo terminale nella tab Console |

Implementazione: un keymap unico con resolver per contesto (`git` | `editor`), deciso
dal focus corrente e non dalla semplice visibilità della tab. Così con la tab Git
visibile ma il cursore nell'editor, `Ctrl+D` resta un comando dell'editor.
Si introduce in F4, insieme alle altre scorciatoie Git.

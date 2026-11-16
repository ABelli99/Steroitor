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

Ultima misura (1.2.0, build release, sessione isolata): installer 2,5 MB · RAM 73,5–79,2 MB · avvio a regime 499–508 ms (1.1.2: RAM 71,8–72,4 MB, avvio 538–690 ms).
Con un repository aperto (29 commit): RAM 79–83 MB. Con una cartella che contiene due repository (`rndProjects`): RAM 80–82 MB, avvio 552–620 ms. Misura post-F6, con un repository di 329 commit e la tab Git visibile: ~95 MB; prima della virtualizzazione del log era ~130 MB.
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

## F7 — Impostazioni e plugin di linguaggio — implementata, in verifica manuale

L'app resta un notepad: i plugin servono a tenere fuori dal nucleo ciò che riguarda un linguaggio solo.

- Pagina **Impostazioni** a tutta finestra (pulsante nella top bar, `Ctrl+Alt+S` come in IntelliJ, Esc per chiudere), con barra laterale:
  - **Editor**: dimensione carattere, interlinea, larghezza tab, indentazione, tab o spazi. Valgono subito, per tutti i progetti della finestra
  - **Plugin**: elenco dei moduli con linguaggi, versione, stato di caricamento e interruttore Attivo
- Un plugin dichiara i linguaggi e le estensioni dei file che gestisce, e porta le estensioni CodeMirror (evidenziazione, completamento, errori, navigazione). Può anche arricchire un linguaggio integrato (es. `html`)
- I plugin attivi si caricano **una volta** all'avvio, dopo che l'editor è usabile: non pesano sul tempo di avvio. Attivare o disattivare un plugin vale dal riavvio successivo
- Primo plugin: **JavaScript / TypeScript** (`src/lib/plugins/javascript`), con dentro tutto ciò che prima stava nel nucleo: evidenziazione con i template inline Angular/Lit, Vai alla dichiarazione (anche dai tag dei componenti nei `.html`), più completamento (parole del file, keyword, snippet) ed errori di sintassi
- Opzione del plugin **Type-check con TypeScript**, spenta di default: errori di tipo del file aperto, con il `tsconfig.json` più vicino e i tipi di `node_modules`. Fuori dal budget RAM: chi la attiva ne accetta il costo

Note di implementazione:
- Il nucleo non conosce più JS/TS: la navigazione passa da una facet CodeMirror (`definitionProvider`) che il plugin mette nelle sue estensioni.
- Senza type-check, errori di sintassi solo per JavaScript: la grammatica Lezer di TypeScript non copre tutto (es. `(x): x is T =>`) e sui file del progetto dava falsi positivi. Con il type-check, gli errori (sintassi e tipi) li dà TypeScript.
- Type-check: language service di TypeScript in un web worker, avviato all'avvio solo se l'opzione è attiva. L'host di TypeScript è sincrono, quindi i file si leggono a giri: il worker annota quelli che mancano, `probe_paths` (Rust) li legge a lotti, si riprova. Su questo repo il primo controllo fa 14 giri e ~950 letture; i successivi 10–20 ms. Le lib di TypeScript sono chunk separati, caricati solo se il progetto le chiede. Il watcher invalida i file cambiati; `node_modules` è escluso dal watcher, quindi dopo un `npm install` serve un riavvio.
- Il type-check guarda il file su disco per gli import: le modifiche non salvate di un'altra tab non si vedono (al cambio tab c'è comunque l'autosave).
- Misura (build release, sessione isolata): installer 2,5 MB → 3,8 MB con il compilatore TypeScript (3,5 MB di worker + lib). RAM con type-check spento invariata (A/B con il commit precedente: 80–85 MB per entrambi, macchina più carica rispetto alla misura del 1.2.0). Type-check attivo: +~18 MB con un file `.txt` (solo motore caricato); con `workspace.svelte.ts` di questo repo ~290 MB a 15 s, ~200 MB a 40 s (il resto era garbage dei giri di caricamento).
- Plugin di terze parti (caricati da una cartella su disco): non ancora. Serve un'API che passi al plugin le istanze CodeMirror dell'app, un manifest e una CSP che permetta script locali.

---

## Riscrittura nativa (branch `native`) — N1–N10 e U1–U7 implementate

Obiettivo: la massima leggerezza. WebView2 da solo vale ~65 dei ~72 MB dell'app Tauri; un'app Win32 con l'editor di Notepad++ (Scintilla) ne usa una frazione. L'app Tauri resta su `master`, usabile, finché la nativa non arriva a parità.

| Area | Scelta | Perché | Scartato |
|---|---|---|---|
| Linguaggio | **Rust** | Riusa il backend esistente (git, watcher, sessioni, ConPTY, LSP); stessa RAM del C++ | C++ (backend da riscrivere, nessun guadagno di memoria) |
| Finestre | **Win32** (`windows-sys`), tab e status bar disegnate con GDI | Il minimo possibile; i controlli comuni non seguono il tema scuro | egui, Slint (editor di codice da scrivere da zero) |
| Editor | **Scintilla 5.6.9 + Lexilla 5.5.5** (sorgenti in `native/vendor`, compilati con `cc`) | Evidenziazione, folding, multi-cursore, completamento già pronti; un documento Scintilla per tab | — |
| Logica condivisa | crate **`core/`** | Usato da entrambe le app mentre si migra: decodifica/codifica dei file, explorer, watcher, ricerca dei repository, tutto il backend Git e il grafo del log | — |

Fasi:
- **N1** — finestra, editor con evidenziazione (16 lexer: C-like per JS/TS/Java/C/C#, Python, JSON, YAML, Markdown, HTML/XML, CSS, Rust, TOML, INI, Batch, PowerShell, SQL, Diff), tab (clic, clic centrale, ×, `Ctrl+Tab`), `Ctrl+N/O/S/Shift+S/W`, file da riga di comando, encoding e BOM preservati, fine riga rilevato, tema chiaro/scuro di sistema, DPI per monitor. **Misura: 2 MB di RAM privata** (19 MB di working set con le DLL di sistema condivise) contro ~72 MB; eseguibile 1,3 MB contro 7 MB
- **N2** — File Explorer (TreeView di Windows, cartelle caricate all'apertura, menu: nuovo file/cartella, rinomina `F2`, cestino `Canc`, mostra in Esplora risorse, copia percorso), cartella di progetto (intestazione dell'explorer, argomento da riga di comando o trascinamento), `Alt+1` explorer e `Alt+Z` a capo come nella versione Tauri, explorer ridimensionabile, watcher (albero aggiornato; file aperti ricaricati se cambiati su disco e senza modifiche), sessione (cartella, tab, finestra) in `%APPDATA%\it.overzoom.steroitor\native`, impostazioni Editor lette dallo stesso `settings.json` della versione Tauri, ricerca e sostituzione (`Ctrl+F`, `Ctrl+H`, `F3`, maiuscole/minuscole, tutte le occorrenze evidenziate), fine riga cliccabile nella status bar, menu scuri. Logica di explorer, watcher e ricerca dei repository spostata in `core/` (la usa anche l'app Tauri). Misura con una cartella aperta: 1,9 MB di RAM privata
- **N3** — Git, con il backend spostato da `src-tauri` in `core/src/git` (stesso codice e test per le due app): stato dei repository della cartella aggiornato dal watcher, explorer colorato come IntelliJ, marker delle righe aggiunte/modificate/tolte rispetto a HEAD, branch con ahead/behind nella status bar e menu VCS (``Alt+` ``: pull col metodo delle impostazioni, push `Ctrl+Shift+K`, fetch, nuovo branch, checkout, merge). Pannello inferiore a tab, ridimensionabile: **Console** con i comandi git eseguiti, **Git** con il log (`Alt+9`), il grafo (algoritmo portato in `core/src/git/graph.rs`), refs, autore, data, hash e menu sul commit (checkout, nuovo branch, tag, cherry-pick, revert, reset soft/mixed/hard con conferma, copia hash), **Commit** (`Ctrl+K`) con i file da spuntare, il messaggio, Commit (`Ctrl+Invio`) e Commit e Push (`Ctrl+Alt+K`); `Ctrl+T` fa il pull col focus nel pannello. Misura: 2,3 MB di RAM privata con il log aperto
- **N4** — terminale nella tab Console. ConPTY chiamata direttamente con `windows-sys` (niente `portable-pty`, che avrebbe portato `winapi` e altre dipendenze); emulatore VT minimo sul crate `vte` (`native/src/vt.rs`: movimenti del cursore, cancellazioni, inserimento/cancellazione di righe e caratteri, regione di scroll, schermo alternativo, colori 16/256/truecolor, risposta alla richiesta di posizione del cursore, bracketed paste); lo schermo è mostrato in un controllo Scintilla in sola lettura, che dà selezione, copia e scroll. Lo scrollback (2000 righe) sta solo nel documento Scintilla: l'emulatore tiene lo schermo e le righe appena uscite in cima. Sotto-tab della Console: "Comandi git" (il log di N3), un tab per terminale con ×, `+`. `Ctrl+T` apre un terminale nella cartella del progetto (col focus nei pannelli Git/Commit resta il pull). Shell dalle impostazioni (`terminalShell`), con il rilevamento spostato in `core/src/shells.rs` e usato da entrambe le app. Col focus nel terminale i tasti vanno alla shell, tranne ``Alt+1/Z/`/9``, `Alt+F4`, `Ctrl+K` e varianti, `Ctrl+T`, `Ctrl+Tab`; `Ctrl+C` copia se c'è una selezione, `Ctrl+V` e `Shift+Ins` incollano. Misura: 3,1 MB di RAM privata con un terminale aperto e 2000 righe di scrollback
- **N5a** — finestra di confronto (una sola, riusata; `native/src/compare.rs`). Diff affiancato con le righe allineate da `similar` (`native/src/diff.rs`), righe di riempimento, numeri di riga reali nel margine, sfondo verde/blu/grigio per righe aggiunte/modificate/tolte, scorrimento sincronizzato, evidenziazione del linguaggio; `F7`/`Shift+F7` e i pulsanti ◀ ▶ passano da una modifica all'altra, `Esc` chiude. Si apre con `Ctrl+D` nel log (file cambiati dal commit, lista a sinistra, diff col primo parent) o nella tab Commit (file rispetto a HEAD), e dai menu "Mostra modifiche" e "Mostra diff". Merge a 3 vie per i file in conflitto (doppio clic o menu "Risolvi conflitti…" nella tab Commit): Nostra (HEAD) | Risultato modificabile | Loro, righe cambiate rispetto alla base segnate sui lati, blocchi di conflitto in rosso e contati nel titolo, Tutte nostre / Tutte loro, Applica e segna risolto (salva con encoding e BOM del file, poi `git add`), conferma se restano conflitti o se si chiude con modifiche
- **N5b** — pagina impostazioni (`Ctrl+Alt+S`, o menu dell'intestazione dell'explorer), modale: Editor come nella versione Tauri (dimensione, interlinea ora applicata con lo spazio extra sopra e sotto le righe, larghezza tab, indentazione, tab o spazi) e, siccome qui non ci sono i menu dove la Tauri li cambia, metodo di update (Merge/Rebase) e shell dei nuovi terminali. Si applica subito a tutte le tab e al confronto; scrive solo le sue chiavi in `settings.json`, che resta condiviso con la versione Tauri
- **N5c** — plugin dei linguaggi (`native/src/plugins`): JavaScript/TypeScript con **tsserver** (via Node: dal `node_modules` del progetto, da `npm i -g typescript` o da VS Code; il TypeScript in JS della versione Tauri non si porta) e Java con **jdtls** (stesso rilevamento e stesso workspace della Tauri). Ogni plugin e ognuna delle sue tre opzioni — completamento, type-check, documentazione inline — si accende dalla pagina impostazioni e vale subito: il server parte al primo file che lo richiede e si chiude quando plugin o opzioni si spengono. Completamento dopo il punto, mentre si scrive o con `Ctrl+Spazio`, con le parole del documento se il server non c'è o non risponde, e richiesto di nuovo a ogni carattere se il server taglia la lista; documentazione della voce in un riquadro accanto alla lista (un calltip di Scintilla la chiuderebbe) e, col mouse fermo, nel calltip; errori e avvisi sottolineati, messaggio col mouse fermo. Chiavi `disabledPlugins` e `pluginOptions` condivise con la Tauri (`typeCheck` uguale; `completion` e `docs` nuove o con un default diverso: qui un'opzione mai toccata è spenta, perché accende un server da 50 MB a 1 GB). Un server che non parte o si chiude finisce nella Console e non si riprova fino al prossimo cambio delle impostazioni dei plugin. Le modifiche aggiuntive di jdtls (gli `import` automatici) non si applicano


Parità con la versione Tauri (cosa manca dopo N5; il backend Git per tutte le voci è già in `core/`, manca l'interfaccia):
- **N6** (fatta) — pannello inferiore sempre raggiungibile: da chiuso resta la striscia Console | Git | Commit, un clic la apre, un clic sulla tab attiva la richiude; pulsante impostazioni nella barra delle tab; scelta del repository di log e commit quando la cartella ne contiene più d'uno
- **N7** (fatta) — più progetti nella stessa finestra: barra a sinistra con l’iniziale di ogni progetto (nome nel tooltip), `+` per aprire una cartella in un progetto nuovo, clic centrale per chiudere; ognuno con explorer, tab, repository, watcher, Console e terminali propri. Il progetto attivo vive nei campi di `App`, gli altri sono parcheggiati e si scambiano al cambio; un progetto ripristinato si carica quando lo apri, e le modifiche viste dal watcher di un progetto non attivo si applicano quando torna attivo. Una cartella da riga di comando o trascinata va nel progetto che la ha già, in quello attivo se è vuoto, altrimenti in uno nuovo. Non ci sono il riordino trascinando e lo stacco in una finestra propria
- **N8** (fatta) — Git. Menu VCS: Continua/Annulla dell’operazione in corso (segnata anche nella status bar), stash (accantona, applica e rimuovi, elimina), rebase del branch corrente, oltre a pull/push/fetch/checkout/merge. Commit: casella per l’amend dell’ultimo commit (propone il suo messaggio, chiede conferma se è già pushato), avviso su HEAD staccato e su file in stage con fine riga CRLF (`warnCrlf` della Tauri). Log: riga dei filtri (branch, autore, percorso, ricerca nel messaggio; il grafo si nasconde con autore, percorso o testo) e Modifica messaggio dei commit non pushati in un campo su più righe. Gutter: clic sul marker per annullare il blocco nell’editor (una modifica, annullabile), metterlo in stage da solo (commit parziale: l’index prende il blocco, il file non cambia), vederne il diff, accendere le annotazioni (blame) in un margine; clic su un’annotazione apre il commit. Intestazione dell’explorer: clona un repository (si apre come progetto) e `git init`. Note: il blame è quello del file salvato, con modifiche non salvate le righe possono non combaciare; il push senza upstream lo imposta da solo, senza chiedere; i comandi git in background restano nascosti nella Console, senza la casella per mostrarli
- **N9** (fatta) — editor e app. Striscia delle modifiche accanto all’editor (verde aggiunta, blu modificata, rosso tolta, in proporzione al file; clic per andare alla riga) e righe tolte in rosso anche nel gutter. Minimap (interruttore nella status bar): seconda vista Scintilla sullo stesso documento, con i marker Git e il tratto visibile; clic e trascinamento scorrono l’editor. Quick Open (`Ctrl+P`) con la ricerca fuzzy della Tauri. Istanza singola: un secondo avvio passa file e cartelle alla finestra aperta (`--new-instance` per aprirne un’altra). La sessione conserva il testo delle tab modificate e senza titolo, quindi l’uscita non chiede più di salvare. Tab riordinabili trascinandole. Vai alla dichiarazione (`Ctrl+B`, come IntelliJ) dal server del plugin, o nel file senza server; `Ctrl+clic` resta ai cursori multipli di Scintilla. Plugin JavaScript/TypeScript: opzioni “HTML inline React (JSX)” e “HTML inline Angular (template)”, colorate con indicatori sopra il lexer C-like (tag, attributi, valori, testo, `{{ }}`), senza server. Note: “Apri con” non è registrato (manca un installer: si sceglie l’eseguibile dal dialogo di Windows); dentro il JSX un apostrofo nel testo può confondere il lexer per il resto della riga
- **N10** (fatta) — viste Git e Commit come nella versione Tauri. Con più repository nella cartella, sopra le due viste c’è una tab per repository con il numero di file cambiati (al posto del menu di scelta di N6). Git: branch a sinistra (Tutti i branch, Locali, Remoti, ad albero sui `/`, cartelle chiuse tranne quella del branch corrente; clic per filtrare il log, menu con Mostra log, Checkout, Merge, Rebase, Nuovo branch da qui), filtri testo/autore/path sopra il log, log con grafo, hash, ref come etichette, autore e data relativa, dettagli del commit selezionato a destra con i file cambiati; menu del commit con le voci della Tauri. Commit: casella = stage vero (`git add` / `git restore --staged`, a metà se il file ha modifiche in stage e fuori), casella in testa per tutti, banner Continua/Annulla durante merge e rebase, Commit o Amend e Commit and Push, anteprima a destra del file selezionato come diff unificato (nella Tauri è affiancato). Senza repository: git init e clona

Stessa UI della versione Tauri (fasi U, dopo un inventario dell'interfaccia di `master`):
- **U1** (fatta) — barra in alto (Nuovo, Apri, Apri cartella, Clona, Nuovo progetto … Impostazioni, Explorer, Pannello), barra dei progetti solo con due o più progetti, tab alte 34 px con ● per i non salvati, doppio clic sulla striscia vuota per un file nuovo, status bar con un branch per repository e i pulsanti Minimap / A capo, pannello nascosto del tutto quando è chiuso, icona dell'app.
- **U2–U3** (fatte) — explorer con l'intestazione (nome del progetto in maiuscolo, nuovo file, aggiorna, comprimi, menu), le icone dei file della Tauri, stato vuoto con Apri cartella e Clona; menu dell'explorer, dell'editor e VCS con le voci della Tauri; History del file nella finestra di confronto con "Solo le zone cambiate".
- **U4** (fatta) — Console come nella Tauri: sotto-tab "Comandi Git", terminali con ×, "+ Terminale" e la freccia con le shell; log dei comandi con orario, durata ed esito, "Comandi in background", Copia e Pulisci.
- **U5** (fatta) — dialoghi: campo di testo senza barra del titolo al 15% dall'alto, Quick Open come palette (nome e cartella per riga), Clona repository con URL, cartella proposta da `cloneParent` + nome del repository, Sfoglia… e avviso sulla cartella.
- **U6** (fatta) — impostazioni a tutta finestra: sezioni a sinistra (Editor, Plugin e, solo qui, Git e terminale per metodo di update e shell, che nella nativa non hanno altri menu), schede dei plugin con versione, descrizione, linguaggi, opzioni descritte. Le modifiche valgono alla chiusura (✕ o Esc), non a ogni clic come nella Tauri.
- **U7** (fatta) — editor senza file aperti: "Nessun file aperto" con Nuovo file e Apri file. Dentro resta una tab vuota nascosta (`placeholder`), così il resto del codice può contare su una tab attiva.
- **Plugin COBOL** (fatto, solo nativa) — `.cbl`, `.cob`, `.cpy` colorati con il lexer COBOL di Lexilla; plugin senza server (`native/src/plugins/cobol.rs`) con due opzioni, accese di default perché non costano niente (regola generale: un'opzione mai toccata è accesa nei plugin senza server, spenta in quelli che avviano un server). "Controllo del sorgente": nel formato fisso indicatore in colonna 7, DIVISION/SECTION/FD/01/77 nell'area A, istruzioni fuori dall'area A, parole tagliate dalla colonna 72; in entrambi i formati ordine delle DIVISION, stringhe non chiuse (con le righe di continuazione), END-IF/END-EVALUATE/END-SEARCH/END-PERFORM, ELSE e WHEN senza apertura, PERFORM in linea senza END-PERFORM, paragrafi di PERFORM/GO TO inesistenti, nomi usati nella PROCEDURE e mai dichiarati (avviso). Copybook di COPY, `EXEC SQL INCLUDE`, `-INC` e `++INCLUDE` letti dalla cartella del file, dalle sottocartelle `copy`, `copybook(s)`, `cpy`, `copylib` e dalle stesse accanto alla cartella superiore, anche annidati; se uno manca o usa REPLACING, nomi e paragrafi non si controllano. Tipi dalle PIC e dalle USAGE: VALUE alfanumerico su un campo numerico (errore) e numerico su uno alfanumerico o editato (avviso), operandi non numerici in ADD/SUBTRACT/MULTIPLY/DIVIDE/COMPUTE (gli editati solo dopo GIVING/REMAINDER o a sinistra di `=`), MOVE di una stringa non numerica in un campo numerico, MOVE in un 88, SET … TO TRUE su un campo che non è un 88. "Indentazione COBOL": Invio va nell'area A o B, un livello in più dopo IF/ELSE/EVALUATE/WHEN/SEARCH/PERFORM in linea, i gruppi dei dati e le frasi come AT END; END-IF, ELSE, WHEN, END-EVALUATE, END-PERFORM, END-SEARCH si riallineano al loro IF/EVALUATE… appena scritti; Tab e Shift+Tab nel formato fisso alle colonne 8, 12, 16…; sempre spazi. Formato libero con `>>SOURCE FORMAT FREE`, `$SET SOURCEFORMAT"FREE"` o una DIVISION prima della colonna 8. Non è un compilatore: i confronti nelle condizioni, le dimensioni dei campi e i nomi riservati di dialetti non elencati non si controllano.
- **Plugin C / C++** (fatto, solo nativa) — `.c`, `.h`, `.cc`, `.cpp`, `.cxx`, `.hpp`, `.hh`, `.hxx` con **clangd**, stesse tre opzioni di Java (completamento, type-check, documentazione inline), spente di default perché avviano un server. clangd si cerca in `CLANGD_PATH`, nel PATH, in `Program Files\LLVM`, nel componente Clang di Visual Studio e nell'estensione clangd di VS Code/Cursor. Un server per progetto: la cartella più vicina con `compile_commands.json` (anche in `build/`), `compile_flags.txt` o `.clangd`, altrimenti quella con `CMakeLists.txt`, `meson.build` o `Makefile`, altrimenti quella del file. Avviato senza indice in background e senza inserimento automatico di `#include`; i `.c` vanno al server come C, il resto come C++. Provato con clangd 23 (LLVM da winget): completamento, firma della voce (clangd non supporta `completionItem/resolve`: dettaglio ed etichetta diventano `size_type size() const`), errori sottolineati; circa 150 MB di RAM su un file singolo.
- **Plugin Java: Spring e Lombok** (fatto, solo nativa) — jdtls parte con `-javaagent:` sul lombok.jar dell'estensione Java di VS Code (o `LOMBOK_JAR`), come fa VS Code: getter, builder e costruttori generati sono completati e non danno errori (provato con `real_server_jdtls_lombok`: senza agente `getName()` di `@Data` non arriva). Due opzioni senza server, `spring` e `lombok`, spente di default come le altre di un plugin con server (`native/src/plugins/annotations.rs`). Le annotazioni contano solo se gli import (espliciti prima, poi `*`) le fanno risalire a Spring o a Lombok. Completamento: nomi dopo `@` (uniti a quelli di jdtls), attributi dentro le parentesi, chiavi di `application*`/`bootstrap*` `.properties`/`.yml` in `@Value("${`. Controllo Lombok: @Data/@Value/@Builder/@Setter… su interfacce, enum e record, @Data con @Value, @NoArgsConstructor con campi final senza valore e senza `force = true`, @Builder con @NoArgs/@RequiredArgsConstructor senza @AllArgsConstructor, @Builder.Default senza valore, @Setter su campo final, @NonNull su primitivi, @Slf4j e simili con un campo `log`, equals/hashCode su classe che estende senza `callSuper`. Controllo Spring: @Autowired su campo final (errore), static o comunque su campo (avviso, come IntelliJ), @Value con `${chiave}` senza default assente dai file application (solo se il progetto ne ha), @Transactional/@Async/@Cacheable… su metodi private/static/final, @Bean private o final e @Configuration final (se non `proxyBeanMethods = false`), stereotipi su interfacce e classi astratte (non @Repository), `{variabili}` del path di classe e metodo contro i `@PathVariable` (si salta con path non letterali, `Map` o `@ModelAttribute`). Non è Spring Tools: niente bean da iniettare, niente profili.
- **Plugin Flutter / Dart** (fatto, solo nativa) — `.dart` con l'analysis server di Dart (`dart language-server --protocol=lsp`), le tre opzioni dei server. dart.exe di Flutter prima (`FLUTTER_ROOT`, `flutter.bat` nel PATH, `~\flutter`, `C:\src\flutter`, `bin\cache\dart-sdk` già scaricato da `flutter doctor`), poi `DART_SDK`, `dart.exe` nel PATH, `C:\tools\dart-sdk`, `Program Files\Dart`. Un server per cartella con `pubspec.yaml`. Evidenziazione con il lexer C++ (stringhe a tre virgolette, `$`). Provato con il server vero (`real_server_dart`, Dart SDK 3 indicato con `DART_SDK`).
- **Plugin Android / Kotlin** (fatto, solo nativa) — `.kt`, `.kts` con **kotlin-language-server** (fwcd), avviato con Java direttamente (`-cp lib\*`, Java come per jdtls) e non con il .bat, così chiudendolo si chiude anche Java; cercato in `KOTLIN_LANGUAGE_SERVER`, nel PATH e nello spazio globale dell'estensione Kotlin di VS Code. Un server per build Gradle (la cartella con `settings.gradle[.kts]`). Opzione senza server **Risorse Android** (`native/src/plugins/android.rs`): in Kotlin e Java dei moduli Android (cartella con `src/main/AndroidManifest.xml`) e negli XML di `res/` e nel manifest, `R.` completa i tipi, `R.<tipo>.` e `@<tipo>/` i nomi; i riferimenti che non sono in nessun `src/<set>/res` del progetto sono avvisi (possono venire da librerie); stili, attributi e styleable non si controllano, `android.R`, `@android:` e `?attr/` si saltano. Le risorse si rileggono a ogni controllo (nessuna cache). Provato con il server vero (`real_server_kotlin`, release 1.3.13 indicata con `KOTLIN_LANGUAGE_SERVER`; si riconosce dalla cartella con `bin\kotlin-language-server.bat` e `lib`). Limite: senza build Gradle o Maven il server cerca la libreria standard di Kotlin solo nelle cache `~/.m2` e `~/.gradle`; se sono vuote ogni `String` è un errore finché il progetto non ha un build o un `kls-classpath.bat` nella radice che stampa il classpath (il test usa quest'ultimo).
- **Plugin Vue** (fatto, solo nativa) — `.vue` (colorati con il lexer HTML) con tsserver e `@vue/typescript-plugin` (`--globalPlugins`/`--pluginProbeLocations`), le tre opzioni dei server: script e template tipizzati come nell'hybrid mode di Volar. Il plugin si cerca nel `node_modules` del progetto, in npm globale (anche dentro `@vue/language-server`) e nell'estensione Vue di VS Code (nome dal suo `package.json`). Il completamento di tag e attributi HTML del template non c'è (lo fa il language server Vue, non tsserver). Provato con il server vero (`real_server_vue`, con `STEROITOR_REAL_BASE` su una cartella con `vue`, `typescript` e `@vue/typescript-plugin` installati).

Note: nel terminale i caratteri larghi (CJK, emoji) occupano una cella sola, i terminali non si riaprono al cambio di cartella né al riavvio e il mouse non arriva alle applicazioni a tutto schermo; il trascinamento di testo dentro l'editor non c'è (Scintilla lo fa via OLE, che non viene inizializzato, così i file trascinati arrivano alla finestra); il cambio di tema di Windows vale dal riavvio; la casella di rinomina dell'explorer resta chiara in tema scuro. Un panic viene scritto in `native\crash.log`.

---

## Fuori scope (decisione esplicita)

Presenti nel CLAUDE.md ma incompatibili con "notepad leggero":

| Feature | Motivo |
|---|---|
| Changelist stile IntelliJ | Duplica la staging area, logica complessa, beneficio basso |
| Interactive Rebase | UI complessa, uso raro; il terminale basta |
| Local History | Richiede storage persistente e indicizzazione continua |
| Integrazione GitHub/GitLab (PR, ecc.) | Dipendenze di rete e auth, fuori dal ruolo di un notepad |
| GPG signing, protected branches, SSH built-in | Si delega a Git di sistema e alla sua config |
| Auto-fetch | Traffico e processi in background non richiesti |

**Eccezione decisa (1.1):** più progetti nella stessa finestra, come tab nella barra laterale sinistra, e
trascinabili fuori in una finestra propria. Ogni progetto ha il suo watcher, il suo stato Git e i suoi
terminali, quindi il costo è quello indicato sopra, moltiplicato per i progetti aperti. Per contenerlo, un
progetto ripristinato all'avvio non si carica finché non lo apri. Il budget RAM vale per un progetto solo.

**Eccezione decisa (post-1.1.0):** più repository nello stesso progetto, es. `gipso/` con dentro
`gipso-fe` e `gipso-be`. Si cercano fino a 3 livelli sotto la cartella, saltando `node_modules`, `target` e
le cartelle nascoste. Ogni repository ha stato, branch, log e commit propri; un file appartiene al
repository più interno che lo contiene. Un solo watcher per progetto, che legge il `.gitignore` di ogni
repository. Non c'è il checkout sincronizzato dello stesso branch su tutti i repository.

**Eccezione decisa (post-1.1.1):** controllo della versione su GitHub. Una sola richiesta per avvio, 5 s dopo
l'apertura e solo dalla finestra principale, alla API delle Release; se ne esiste una più nuova compare un
avviso nella status bar, che apre la pagina della release o ignora quella versione. Non scarica e non installa
nulla da solo. La richiesta parte dalla webview (CSP: solo `api.github.com`), così non serve un client HTTP in
Rust, che costava +0,5 MB di installer.

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

# CLAUDE.md — Mini-IDE con Git (File Explorer + Editor + Console + Git History)

> Specifica di progetto per ricostruire da zero un’applicazione tipo IDE leggera.
> Include: File Explorer, Text Editor, pannelli inferiori (Console + Git Branches/History),
> e l’intera integrazione Git modellata sul comportamento di IntelliJ IDEA.
>
> Uso: esercizio di apprendimento. Non destinato a distribuzione commerciale.
> Lingua preferita delle risposte: **italiano**, salvo diversa richiesta.

---

## Scopo del progetto

Il progetto è una **mini-IDE** (o estensione / app desktop / web-app stile IDE) che deve fornire:

1. **File Explorer** (albero dei file del progetto)
2. **Text Editor** (editor di testo multi-tab, con evidenziazione di base)
3. **Pannello inferiore a tab**, con almeno:
   - **Console** (una o più console / terminali)
   - **Git** (visione branch + history: commit id, data/ora, autore, messaggio, grafo)
4. Tutta la conoscenza e il comportamento Git descritti sotto (ispirati a IntelliJ IDEA)

Claude (o chi legge questo file) deve poter:

- ricostruire l’architettura UI e le funzionalità da zero
- implementare o spiegare ogni componente
- mantenere coerenza con i comportamenti Git di IntelliJ descritti nelle sezioni successive

---

## Architettura UI obbligatoria

### Layout generale (stile IDE classico)

```
┌─────────────────────────────────────────────────────────────────┐
│  Menu bar / Toolbar (File, Edit, View, Git, …) + VCS widget     │
├──────────────┬──────────────────────────────────────────────────┤
│              │                                                  │
│  File        │              Text Editor                         │
│  Explorer    │         (tab aperte dei file)                    │
│  (albero)    │                                                  │
│              │                                                  │
│              │                                                  │
├──────────────┴──────────────────────────────────────────────────┤
│  Pannello inferiore (tab):                                      │
│  [ Console ]  [ Git ]  [ … altre tab eventuali ]                │
│                                                                 │
│  contenuto della tab attiva                                     │
└─────────────────────────────────────────────────────────────────┘
```

### 1. File Explorer (obbligatorio)

- Posizione: **colonna sinistra** (ridimensionabile, collassabile).
- Mostra l’albero del filesystem del progetto (cartelle e file).
- Comportamenti minimi:
  - Espandi / collassa cartelle
  - Click su file → apre il file nel **Text Editor** (nuova tab o focus su tab esistente)
  - Click destro (context menu): New File, New Folder, Rename, Delete, Reveal in OS, Copy Path, ecc.
  - Indicatori di stato Git sui file (stessi colori di IntelliJ, vedi sotto):
    - Verde = nuovo / aggiunto
    - Blu = modificato
    - Grigio / rosso = untracked o ignored
  - Refresh automatico o manuale quando cambiano i file su disco o dopo operazioni Git
- Supporto drag-and-drop (opzionale ma consigliato): spostare file/cartelle.

### 2. Text Editor (obbligatorio)

- Posizione: **area centrale**.
- Multi-tab: ogni file aperto è una tab (chiudibile, riordinabile).
- Funzionalità minime:
  - Apertura / salvataggio file
  - Editing testo (undo/redo, selezione, copia/incolla)
  - Evidenziazione sintassi di base (almeno per linguaggi comuni: JS/TS, Python, Java, Markdown, JSON, ecc.)
  - Numeri di riga
  - Gutter con marker di modifica Git (come in IntelliJ):
    - marker colorati sulle righe cambiate
    - click sul marker → diff / stage / rollback riga (se implementato)
  - Indicatore “dirty” (file non salvato) sulla tab
- Opzionale ma utile: minimap, word wrap, ricerca nel file (`Ctrl+F`).

### 3. Pannello inferiore a tab (obbligatorio)

Posizione: **parte bassa** della finestra (ridimensionabile, collassabile).  
Deve contenere **almeno** queste due tab:

#### Tab “Console” (o “Terminal” / “Consoles”)

- Una o più console/terminali.
- Comportamenti:
  - Output di comandi eseguiti dall’app (in particolare i comandi Git)
  - Possibilità di avere più istanze (tab secondarie o split) se utile
  - Input interattivo se è un vero terminale (shell), oppure solo output se è una console di log
  - Scroll, clear, copia output
- Deve mostrare l’output reale dei comandi Git eseguiti sotto il cofano (come la Console del Git tool window di IntelliJ).

#### Tab “Git” (Branches + History)

Vista dedicata a Git, ispirata al **Git tool window → Log** di IntelliJ.

Deve mostrare:

| Elemento | Descrizione |
|----------|-------------|
| **Branch list** | Branch locali e remote (es. `main`, `origin/main`, feature branches). Branch corrente evidenziato. |
| **History / Log** | Lista o grafo dei commit del branch selezionato (o di tutti i branch). |
| **Commit ID** | Hash breve e/o completo (es. `a1b2c3d` / full SHA). |
| **Time / Date** | Data e ora del commit (relativa e/o assoluta). |
| **Author** | Nome e/o email dell’autore. |
| **Message** | Messaggio di commit (prima riga + body su espansione). |
| **Grafo** | Linee/branch graph (opzionale ma fortemente consigliato) che mostra merge e divergenze. |

Azioni tipiche dalla tab Git (click destro o toolbar):

- Checkout branch / commit
- New Branch
- Merge / Rebase
- Cherry-Pick
- Revert
- Reset (soft / mixed / hard)
- Create Tag
- Edit Commit Message (se non pushato)
- Copy Commit ID
- Show Diff / Show Changes del commit
- Push / Pull / Fetch (o delegare alla toolbar principale)

Filtri utili: per branch, per autore, per path, ricerca testo nel messaggio.

---

## Componenti aggiuntivi consigliati (allineati a IntelliJ)

### VCS Widget (barra superiore / status bar)

- Mostra il **branch corrente**.
- Click → popup Branches (New Branch, Checkout, Merge, Rebase, Push, Pull, Fetch, Compare, …).

### Commit area (opzionale ma raccomandata)

Può essere:

- una tab aggiuntiva nel pannello inferiore, oppure
- un pannello laterale / dialog dedicato

Contenuto ispirato al **Commit tool window** di IntelliJ:

- Lista file modificati / unversioned / cancellati
- Checkbox di selezione
- Messaggio di commit
- Pulsanti Commit / Commit and Push
- Diff preview
- Solo staging area (niente changelist)

### Indicatori visivi (obbligatori dove applicabile)

| Colore / elemento | Significato |
|-------------------|-------------|
| Verde (file nel File Explorer) | Nuovo, aggiunto a Git |
| Blu (file) | Modificato |
| Grigio / rosso | Untracked o ignored |
| Marker nel gutter dell’Editor | Riga modificata |
| Annotate / Blame | Autore e data per riga (click destro → Annotate) |

---

## Integrazione Git — comportamento di riferimento (IntelliJ)

L’app **non reimplementa Git**: esegue i comandi Git reali (o una libreria equivalente, es. isomorphic-git / libgit2 / simple-git) e mostra l’output nella tab **Console**.

### Configurazione Git

- Path all’eseguibile Git (o rilevamento automatico)
- SSH e credenziali delegati a Git di sistema
- Opzioni analoghe a IntelliJ:
  - Update method: **Merge** (default) o **Rebase**
  - Credential helper (delegato a Git di sistema)
  - Warn su CRLF, detached HEAD, rebase

### Abilitare Git nel progetto

1. Clonare un repository remoto
2. `git init` su cartella esistente
3. Aprire una cartella che contiene già `.git` → rilevamento automatico

Dopo l’abilitazione devono comparire: VCS widget, indicatori colori, tab Git popolata, azioni Commit/Push/ecc.

### Operazioni e mappa comandi

| Azione UI | Comando Git equivalente (semplificato) |
|-----------|----------------------------------------|
| Add | `git add` |
| Commit | `git commit -m "..."` |
| Push | `git push` |
| Pull (Merge) | `git pull` / `git fetch` + `git merge` |
| Pull (Rebase) | `git pull --rebase` / `git fetch` + `git rebase` |
| Checkout branch | `git checkout` / `git switch` |
| New Branch | `git checkout -b` / `git switch -c` |
| Merge | `git merge` |
| Rebase | `git rebase` |
| Cherry-Pick | `git cherry-pick` |
| Revert | `git revert` |
| Reset | `git reset` (soft/mixed/hard) |
| Stash | `git stash` |
| Annotate / Blame | `git blame` |
| Log / History | `git log` (con opzioni grafo se supportato) |

### Scorciatoie di riferimento (Windows/Linux; su macOS `Ctrl` → `Cmd`)

| Operazione | Scorciatoia tipica |
|------------|--------------------|
| Commit | `Ctrl+K` |
| Commit and Push | `Ctrl+Alt+K` |
| Push | `Ctrl+Shift+K` |
| Add | `Ctrl+Alt+A` |
| Focus Git panel | `Alt+9` (o equivalente) |
| Focus Console | scorciatoia dedicata |
| Show Diff | `Ctrl+D` (solo con focus su tab Git/Commit) |
| Update / Pull (contestuale) | `Ctrl+T` (solo con focus su tab Git/Commit) |
| VCS popup | `Alt+\`` |

**Scorciatoie contestuali:** `Ctrl+D` e `Ctrl+T` eseguono l'azione Git solo quando il focus
è nella tab Git o nel pannello Commit. Con il focus nell'editor (o altrove) valgono
rispettivamente come "seleziona occorrenza successiva" e "nuovo terminale" (apre una console
e ci sposta il focus).

### Workflow tipici da supportare

**Setup da zero**

1. Rilevare o configurare Git
2. Clonare / init / aprire repo esistente
3. File Explorer mostra file con stati Git
4. Tab Git mostra branch e history

**Commit e Push**

1. Modifica file nell’Editor → marker blu/verde
2. Seleziona file (Commit area o staging)
3. Messaggio → Commit o Commit and Push
4. Output in Console; history aggiornata nella tab Git

**Branch e merge**

1. Nuovo branch da VCS widget o tab Git
2. Lavoro + commit
3. Checkout su main + Merge
4. Conflitti: tool a 3 vie (Yours / Result / Theirs) se implementato

**History**

1. Tab Git → seleziona branch
2. Vedi commit id, time, author, message, grafo
3. Click destro → Cherry-Pick, Revert, Reset, Diff, ecc.

---

## Funzionalità avanzate

L'app è un **notepad potenziato**, non un IDE completo: pianificazione, budget di
performance e perimetro sono in `ROADMAP.md`, che prevale su questo documento in caso di conflitto.

In roadmap o backlog: commit parziale, amend, edit commit message, stash, merge tool a 3 vie,
terminale interattivo.

**Fuori scope:** changelist (solo staging area), interactive rebase, Local History,
multi-repository, integrazione GitHub/GitLab, GPG signing, protected branches,
SSH built-in, auto-fetch.

---

## Principi per chi implementa / per Claude

1. **UI first**: File Explorer a sinistra, Editor al centro, pannello inferiore con tab Console + Git.
2. La tab **Git** deve sempre mostrare almeno: branch, commit id, time, author, message (e idealmente il grafo).
3. La tab **Console** deve ricevere l’output dei comandi Git eseguiti.
4. I colori file e i marker gutter seguono la convenzione IntelliJ (verde / blu / untracked).
5. Preferire sempre azioni UI + scorciatoie rispetto al solo comando terminale grezzo.
6. Non inventare scorciatoie non elencate; in caso di dubbio indicare l’equivalente IntelliJ.
7. Risposte all’utente in **italiano**, strutturate e precise.
8. Se si chiede di “ricostruire da zero”, partire da questa architettura UI e poi riempire i comportamenti Git.

---

## Checklist “ricostruzione da zero”

Chi legge solo questo CLAUDE.md deve poter:

- [ ] Disegnare il layout: File Explorer | Editor | pannello inferiore (Console + Git)
- [ ] Implementare o descrivere File Explorer con stati Git colorati
- [ ] Implementare o descrivere Text Editor multi-tab con gutter marker
- [ ] Implementare tab **Console** (output comandi)
- [ ] Implementare tab **Git** con branch list + history (commit id, time, author, message, grafo)
- [ ] Collegare le azioni UI ai comandi Git (add, commit, push, pull, branch, merge, …)
- [ ] Spiegare configurazione Git, indicatori, workflow commit/branch/history
- [ ] Rispondere in italiano in modo coerente con questo documento

---

## Fonti di riferimento (comportamento Git)

- JetBrains IntelliJ IDEA – Settings Git, Commit, Log, tutorial ufficiali
- Comportamento reale: i comandi eseguiti sono quelli di Git; l’UI è modellata su IntelliJ

Fine del contesto CLAUDE.md.

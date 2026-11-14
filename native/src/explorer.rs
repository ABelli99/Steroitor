//! File Explorer: TreeView di Windows, cartelle caricate quando si aprono, aggiornate dal watcher.
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, POINT};
use windows_sys::Win32::Graphics::Gdi::{ScreenToClient, HFONT};
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use steroitor_core::fs;

use crate::dialogs::{copy_to_clipboard, message_box, wide};
use crate::theme::Palette;

/// Cosa l'app deve fare dopo un'azione nell'explorer.
pub enum Action {
    Open(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
    Deleted(PathBuf),
    /// Nuovo file o cartella in questa cartella: il nome lo chiede l'app.
    Create { dir: PathBuf, folder: bool },
    Rename(PathBuf),
    Blame(PathBuf),
    History(PathBuf),
}

struct Node {
    path: PathBuf,
    is_dir: bool,
    item: HTREEITEM,
    loaded: bool,
}

pub struct Explorer {
    hwnd: Cell<HWND>,
    root: RefCell<Option<PathBuf>>,
    nodes: RefCell<Vec<Node>>,
}

const MENU_NEW_FILE: usize = 1;
const MENU_NEW_FOLDER: usize = 2;
const MENU_RENAME: usize = 3;
const MENU_DELETE: usize = 4;
const MENU_REVEAL: usize = 5;
const MENU_COPY_PATH: usize = 6;
const MENU_BLAME: usize = 7;
const MENU_HISTORY: usize = 8;
const MENU_REFRESH: usize = 9;

fn send(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> isize {
    // SAFETY: messaggi TreeView verso una finestra valida, con puntatori a strutture che vivono per la chiamata.
    unsafe { SendMessageW(hwnd, message, wparam, lparam) }
}

impl Explorer {
    pub fn create(parent: HWND, instance: HINSTANCE, palette: &Palette) -> Explorer {
        let class = wide("SysTreeView32");
        let style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | (TVS_HASBUTTONS | TVS_LINESATROOT | TVS_SHOWSELALWAYS | TVS_FULLROWSELECT) as u32;
        // SAFETY: creazione di un controllo figlio della finestra principale.
        let hwnd = unsafe {
            let controls = INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_TREEVIEW_CLASSES };
            InitCommonControlsEx(&controls);
            CreateWindowExW(0, class.as_ptr(), std::ptr::null(), style, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null())
        };
        // Il tema di Esplora risorse dà le frecce al posto dei +/− (anche in chiaro).
        let theme = wide(if palette.dark { "DarkMode_Explorer" } else { "Explorer" });
        // SAFETY: controllo valido, stringa terminata da zero.
        unsafe { SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null()) };
        send(hwnd, TVM_SETEXTENDEDSTYLE, TVS_EX_DOUBLEBUFFER as usize, TVS_EX_DOUBLEBUFFER as isize);
        send(hwnd, TVM_SETBKCOLOR, 0, palette.panel as isize);
        send(hwnd, TVM_SETTEXTCOLOR, 0, palette.fg as isize);
        Explorer { hwnd: Cell::new(hwnd), root: RefCell::new(None), nodes: RefCell::new(Vec::new()) }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd.get()
    }

    /// Scambia controllo e contenuto con un altro explorer: così cambia il progetto mostrato.
    pub fn swap(&self, other: &Explorer) {
        self.hwnd.swap(&other.hwnd);
        self.root.swap(&other.root);
        self.nodes.swap(&other.nodes);
    }

    pub fn set_font(&self, font: HFONT, item_height: i32, indent: i32) {
        send(self.hwnd(), WM_SETFONT, font as usize, 1);
        send(self.hwnd(), TVM_SETITEMHEIGHT, item_height as usize, 0);
        send(self.hwnd(), TVM_SETINDENT, indent as usize, 0);
    }

    pub fn set_image_list(&self, list: HIMAGELIST) {
        send(self.hwnd(), TVM_SETIMAGELIST, TVSIL_NORMAL as usize, list as isize);
    }

    pub fn root(&self) -> Option<PathBuf> {
        self.root.borrow().clone()
    }

    pub fn set_root(&self, root: Option<&Path>) {
        send(self.hwnd(), TVM_DELETEITEM, 0, TVI_ROOT as isize);
        self.nodes.borrow_mut().clear();
        *self.root.borrow_mut() = root.map(Path::to_path_buf);
        if let Some(root) = root {
            self.fill(TVI_ROOT, root);
        }
    }

    fn insert(&self, parent: HTREEITEM, after: HTREEITEM, entry: &fs::Entry) -> HTREEITEM {
        let index = self.nodes.borrow().len();
        let mut text = wide(&entry.name);
        // SAFETY: struttura inizializzata a zero e poi compilata; il testo vive per tutta la chiamata.
        let item = unsafe {
            let mut insert: TVINSERTSTRUCTW = std::mem::zeroed();
            insert.hParent = parent;
            insert.hInsertAfter = after;
            let (image, expanded) = crate::icons::image_of(Path::new(&entry.path), entry.is_dir);
            insert.Anonymous.itemex.mask = TVIF_TEXT | TVIF_PARAM | TVIF_CHILDREN | TVIF_IMAGE | TVIF_SELECTEDIMAGE | TVIF_EXPANDEDIMAGE;
            insert.Anonymous.itemex.iImage = image;
            insert.Anonymous.itemex.iSelectedImage = image;
            insert.Anonymous.itemex.iExpandedImage = expanded;
            insert.Anonymous.itemex.pszText = text.as_mut_ptr();
            insert.Anonymous.itemex.lParam = index as LPARAM;
            insert.Anonymous.itemex.cChildren = i32::from(entry.is_dir);
            send(self.hwnd(), TVM_INSERTITEMW, 0, &insert as *const _ as isize) as HTREEITEM
        };
        self.nodes.borrow_mut().push(Node { path: PathBuf::from(&entry.path), is_dir: entry.is_dir, item, loaded: false });
        item
    }

    fn fill(&self, parent: HTREEITEM, dir: &Path) {
        let Ok(entries) = fs::list_dir(dir.to_string_lossy().into_owned()) else { return };
        for entry in &entries {
            self.insert(parent, TVI_LAST, entry);
        }
    }

    fn node_of(&self, item: HTREEITEM) -> Option<usize> {
        if item == 0 {
            return None;
        }
        // SAFETY: lettura del solo lParam dell'elemento.
        let index = unsafe {
            let mut data: TVITEMEXW = std::mem::zeroed();
            data.mask = TVIF_PARAM;
            data.hItem = item;
            send(self.hwnd(), TVM_GETITEMW, 0, &mut data as *mut _ as isize);
            data.lParam as usize
        };
        (index < self.nodes.borrow().len()).then_some(index)
    }

    /// Percorso della voce con questo lParam (le notifiche di disegno riportano il lParam, non l'elemento).
    pub fn path_of_param(&self, param: LPARAM) -> Option<PathBuf> {
        self.nodes.borrow().get(param as usize).map(|node| node.path.clone())
    }

    fn load(&self, index: usize) {
        let (item, path, loaded) = {
            let node = &self.nodes.borrow()[index];
            (node.item, node.path.clone(), node.loaded)
        };
        if loaded {
            return;
        }
        self.nodes.borrow_mut()[index].loaded = true;
        self.fill(item, &path);
    }

    fn children(&self, parent: HTREEITEM) -> Vec<HTREEITEM> {
        let mut items = Vec::new();
        let mut item = send(self.hwnd(), TVM_GETNEXTITEM, TVGN_CHILD as usize, parent as isize) as HTREEITEM;
        while item != 0 {
            items.push(item);
            item = send(self.hwnd(), TVM_GETNEXTITEM, TVGN_NEXT as usize, item as isize) as HTREEITEM;
        }
        items
    }

    /// Ricarica una cartella già caricata: toglie ciò che non c'è più, inserisce il nuovo al suo posto, tiene il resto (e le sottocartelle aperte).
    pub fn refresh_dir(&self, dir: &Path) {
        let is_root = self.root.borrow().as_deref() == Some(dir);
        let parent = if is_root {
            TVI_ROOT
        } else {
            match self.nodes.borrow().iter().find(|node| node.loaded && node.item != 0 && node.path == dir) {
                Some(node) => node.item,
                None => return,
            }
        };
        let Ok(entries) = fs::list_dir(dir.to_string_lossy().into_owned()) else { return };
        let existing: Vec<(HTREEITEM, PathBuf)> = self
            .children(parent)
            .into_iter()
            .filter_map(|item| self.node_of(item).map(|index| (item, self.nodes.borrow()[index].path.clone())))
            .collect();
        for (item, path) in &existing {
            if !entries.iter().any(|entry| Path::new(&entry.path) == path) {
                send(self.hwnd(), TVM_DELETEITEM, 0, *item as isize);
                if let Some(node) = self.nodes.borrow_mut().iter_mut().find(|node| node.item == *item) {
                    node.item = 0;
                }
            }
        }
        let mut previous = TVI_FIRST;
        for entry in &entries {
            previous = match existing.iter().find(|(_, path)| path == Path::new(&entry.path)) {
                Some((item, _)) => *item,
                None => self.insert(parent, previous, entry),
            };
        }
    }

    fn item_at_cursor(&self) -> HTREEITEM {
        // SAFETY: hit test con coordinate client del controllo.
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point);
            ScreenToClient(self.hwnd(), &mut point);
            let mut hit: TVHITTESTINFO = std::mem::zeroed();
            hit.pt = point;
            send(self.hwnd(), TVM_HITTEST, 0, &mut hit as *mut _ as isize) as HTREEITEM
        }
    }

    fn selected(&self) -> Option<usize> {
        self.node_of(send(self.hwnd(), TVM_GETNEXTITEM, TVGN_CARET as usize, 0) as HTREEITEM)
    }

    /// Seleziona la voce di `path` (aprendo la cartella che la contiene, se è caricata) e la porta in vista.
    pub fn reveal(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let folder = self.nodes.borrow().iter().position(|node| node.path == parent && node.item != 0);
            if let Some(index) = folder {
                self.load(index);
                send(self.hwnd(), TVM_EXPAND, TVE_EXPAND as usize, self.nodes.borrow()[index].item as isize);
            }
        }
        let item = self.nodes.borrow().iter().find(|node| node.path == path && node.item != 0).map(|node| node.item);
        if let Some(item) = item {
            send(self.hwnd(), TVM_SELECTITEM, TVGN_CARET as usize, item as isize);
            send(self.hwnd(), TVM_ENSUREVISIBLE, 0, item as isize);
        }
    }

    /// Ricarica tutte le cartelle aperte (pulsante ⟳).
    pub fn refresh_all(&self) {
        let Some(root) = self.root() else { return };
        self.refresh_dir(&root);
        let open: Vec<PathBuf> = self.nodes.borrow().iter().filter(|node| node.is_dir && node.loaded && node.item != 0).map(|node| node.path.clone()).collect();
        for dir in open {
            self.refresh_dir(&dir);
        }
    }

    /// Chiude tutte le cartelle (pulsante ⊟).
    pub fn collapse_all(&self) {
        let items: Vec<HTREEITEM> = self.nodes.borrow().iter().filter(|node| node.is_dir && node.item != 0).map(|node| node.item).collect();
        for item in items {
            send(self.hwnd(), TVM_EXPAND, TVE_COLLAPSE as usize, item as isize);
        }
    }

    /// Gestisce le notifiche della TreeView; il valore restituito va al chiamante di WM_NOTIFY.
    /// `in_repo` dice se un file sta in un repository Git (per Annotate e Mostra history nel menu).
    pub fn on_notify(&self, owner: HWND, header: &NMHDR, lparam: LPARAM, in_repo: &dyn Fn(&Path) -> bool) -> (isize, Option<Action>) {
        match header.code {
            TVN_ITEMEXPANDINGW => {
                // SAFETY: per questa notifica lParam punta a NMTREEVIEWW.
                let view = unsafe { &*(lparam as *const NMTREEVIEWW) };
                if let Some(index) = self.node_of(view.itemNew.hItem) {
                    self.load(index);
                }
                (0, None)
            }
            // Un clic apre il file o apre/chiude la cartella, come nella versione Tauri; la freccia la gestisce la TreeView.
            NM_CLICK => {
                let (item, on_button) = self.hit_at_cursor();
                let Some(index) = self.node_of(item).filter(|_| !on_button) else { return (0, None) };
                send(self.hwnd(), TVM_SELECTITEM, TVGN_CARET as usize, item as isize);
                let (is_dir, path) = {
                    let node = &self.nodes.borrow()[index];
                    (node.is_dir, node.path.clone())
                };
                if is_dir {
                    self.load(index);
                    send(self.hwnd(), TVM_EXPAND, TVE_TOGGLE as usize, item as isize);
                    return (1, None);
                }
                (1, Some(Action::Open(path)))
            }
            NM_RETURN => {
                let Some(index) = self.selected() else { return (0, None) };
                let (is_dir, path, item) = {
                    let node = &self.nodes.borrow()[index];
                    (node.is_dir, node.path.clone(), node.item)
                };
                if is_dir {
                    self.load(index);
                    send(self.hwnd(), TVM_EXPAND, TVE_TOGGLE as usize, item as isize);
                    return (1, None);
                }
                (1, Some(Action::Open(path)))
            }
            NM_RCLICK => {
                let item = self.item_at_cursor();
                if item != 0 {
                    send(self.hwnd(), TVM_SELECTITEM, TVGN_CARET as usize, item as isize);
                }
                (1, self.context_menu(owner, self.node_of(item), in_repo))
            }
            _ => (0, None),
        }
    }

    fn hit_at_cursor(&self) -> (HTREEITEM, bool) {
        // SAFETY: hit test con coordinate client del controllo.
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point);
            ScreenToClient(self.hwnd(), &mut point);
            let mut hit: TVHITTESTINFO = std::mem::zeroed();
            hit.pt = point;
            let item = send(self.hwnd(), TVM_HITTEST, 0, &mut hit as *mut _ as isize) as HTREEITEM;
            (item, hit.flags & TVHT_ONITEMBUTTON != 0)
        }
    }

    fn refresh_parent(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            self.refresh_dir(parent);
        }
    }

    /// Cartella in cui creare: quella selezionata, la cartella del file selezionato o la radice.
    fn target_dir(&self, node: Option<usize>) -> Option<PathBuf> {
        match node {
            Some(index) => {
                let node = &self.nodes.borrow()[index];
                if node.is_dir {
                    Some(node.path.clone())
                } else {
                    node.path.parent().map(Path::to_path_buf)
                }
            }
            None => self.root(),
        }
    }

    /// Menu come nella versione Tauri; su un file in un repository anche Annotate e Mostra history.
    fn context_menu(&self, owner: HWND, node: Option<usize>, in_repo: &dyn Fn(&Path) -> bool) -> Option<Action> {
        let file = node.map(|index| {
            let node = &self.nodes.borrow()[index];
            (node.path.clone(), node.is_dir)
        });
        let mut entries = vec![(MENU_NEW_FILE, "Nuovo file…"), (MENU_NEW_FOLDER, "Nuova cartella…"), (0, "")];
        match &file {
            Some((path, is_dir)) => {
                entries.extend([(MENU_RENAME, "Rinomina…"), (MENU_DELETE, "Elimina"), (0, ""), (MENU_REVEAL, "Mostra in Esplora risorse"), (MENU_COPY_PATH, "Copia percorso")]);
                if !is_dir && in_repo(path) {
                    entries.extend([(0, ""), (MENU_BLAME, "Annotate con Git Blame"), (MENU_HISTORY, "Mostra history")]);
                }
            }
            None => entries.extend([(MENU_REVEAL, "Mostra in Esplora risorse"), (MENU_COPY_PATH, "Copia percorso"), (0, ""), (MENU_REFRESH, "Aggiorna")]),
        }
        let command = popup_menu(owner, &entries);
        let path = file.as_ref().map(|(path, _)| path.clone()).or_else(|| self.root())?;
        match command {
            MENU_NEW_FILE | MENU_NEW_FOLDER => Some(Action::Create { dir: self.target_dir(node)?, folder: command == MENU_NEW_FOLDER }),
            MENU_RENAME => Some(Action::Rename(path)),
            MENU_DELETE => self.delete(owner, &path),
            MENU_REVEAL => {
                let _ = fs::reveal_in_os(path.to_string_lossy().into_owned());
                None
            }
            MENU_COPY_PATH => {
                copy_to_clipboard(owner, &path.to_string_lossy());
                None
            }
            MENU_BLAME => Some(Action::Blame(path)),
            MENU_HISTORY => Some(Action::History(path)),
            MENU_REFRESH => {
                self.refresh_all();
                None
            }
            _ => None,
        }
    }

    pub fn delete_selected(&self, owner: HWND) -> Option<Action> {
        let path = self.nodes.borrow()[self.selected()?].path.clone();
        self.delete(owner, &path)
    }

    pub fn rename_selected(&self) -> Option<Action> {
        let index = self.selected()?;
        Some(Action::Rename(self.nodes.borrow()[index].path.clone()))
    }

    fn delete(&self, owner: HWND, path: &Path) -> Option<Action> {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned());
        let kind = if path.is_dir() { "la cartella" } else { "il file" };
        if message_box(owner, &format!("Spostare nel Cestino {kind} \"{name}\"?"), MB_YESNO | MB_ICONWARNING) != IDYES {
            return None;
        }
        if let Err(error) = fs::delete_to_trash(path.to_string_lossy().into_owned()) {
            message_box(owner, &error, MB_ICONERROR);
            return None;
        }
        self.refresh_parent(path);
        Some(Action::Deleted(path.to_path_buf()))
    }

    /// Cartella in cui crea il pulsante + dell'intestazione: quella selezionata, quella del file selezionato o la radice.
    pub fn create_target(&self) -> Option<PathBuf> {
        self.target_dir(self.selected())
    }
}

/// Menu contestuale alla posizione del mouse; restituisce l'id scelto, 0 se annullato.
pub fn popup_menu(owner: HWND, entries: &[(usize, &str)]) -> usize {
    // SAFETY: menu creato, mostrato e distrutto qui; le stringhe vivono per tutta la chiamata.
    unsafe {
        let menu = CreatePopupMenu();
        let labels: Vec<Vec<u16>> = entries.iter().map(|(_, label)| wide(label)).collect();
        for ((id, _), label) in entries.iter().zip(&labels) {
            if *id == 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            } else if *id == usize::MAX {
                // Etichetta di sezione, come le voci disattivate dei menu della versione Tauri.
                AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, label.as_ptr());
            } else {
                AppendMenuW(menu, MF_STRING, *id, label.as_ptr());
            }
        }
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, owner, std::ptr::null());
        DestroyMenu(menu);
        command as usize
    }
}


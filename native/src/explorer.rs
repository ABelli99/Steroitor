//! File Explorer: TreeView di Windows, cartelle caricate quando si aprono, aggiornate dal watcher.
use std::cell::RefCell;
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
}

struct Node {
    path: PathBuf,
    is_dir: bool,
    item: HTREEITEM,
    loaded: bool,
}

pub struct Explorer {
    pub hwnd: HWND,
    root: RefCell<Option<PathBuf>>,
    nodes: RefCell<Vec<Node>>,
}

const MENU_NEW_FILE: usize = 1;
const MENU_NEW_FOLDER: usize = 2;
const MENU_RENAME: usize = 3;
const MENU_DELETE: usize = 4;
const MENU_REVEAL: usize = 5;
const MENU_COPY_PATH: usize = 6;

fn send(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> isize {
    // SAFETY: messaggi TreeView verso una finestra valida, con puntatori a strutture che vivono per la chiamata.
    unsafe { SendMessageW(hwnd, message, wparam, lparam) }
}

impl Explorer {
    pub fn create(parent: HWND, instance: HINSTANCE, palette: &Palette) -> Explorer {
        let class = wide("SysTreeView32");
        let style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | (TVS_HASBUTTONS | TVS_LINESATROOT | TVS_SHOWSELALWAYS | TVS_EDITLABELS | TVS_FULLROWSELECT) as u32;
        // SAFETY: creazione di un controllo figlio della finestra principale.
        let hwnd = unsafe {
            let controls = INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_TREEVIEW_CLASSES };
            InitCommonControlsEx(&controls);
            CreateWindowExW(0, class.as_ptr(), std::ptr::null(), style, 0, 0, 0, 0, parent, std::ptr::null_mut(), instance, std::ptr::null())
        };
        if palette.dark {
            let theme = wide("DarkMode_Explorer");
            // SAFETY: controllo valido, stringa terminata da zero.
            unsafe { SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null()) };
        }
        send(hwnd, TVM_SETEXTENDEDSTYLE, TVS_EX_DOUBLEBUFFER as usize, TVS_EX_DOUBLEBUFFER as isize);
        send(hwnd, TVM_SETBKCOLOR, 0, palette.panel as isize);
        send(hwnd, TVM_SETTEXTCOLOR, 0, palette.fg as isize);
        Explorer { hwnd, root: RefCell::new(None), nodes: RefCell::new(Vec::new()) }
    }

    pub fn set_font(&self, font: HFONT, item_height: i32) {
        send(self.hwnd, WM_SETFONT, font as usize, 1);
        send(self.hwnd, TVM_SETITEMHEIGHT, item_height as usize, 0);
    }

    pub fn root(&self) -> Option<PathBuf> {
        self.root.borrow().clone()
    }

    pub fn set_root(&self, root: Option<&Path>) {
        send(self.hwnd, TVM_DELETEITEM, 0, TVI_ROOT as isize);
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
            insert.Anonymous.itemex.mask = TVIF_TEXT | TVIF_PARAM | TVIF_CHILDREN;
            insert.Anonymous.itemex.pszText = text.as_mut_ptr();
            insert.Anonymous.itemex.lParam = index as LPARAM;
            insert.Anonymous.itemex.cChildren = i32::from(entry.is_dir);
            send(self.hwnd, TVM_INSERTITEMW, 0, &insert as *const _ as isize) as HTREEITEM
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
            send(self.hwnd, TVM_GETITEMW, 0, &mut data as *mut _ as isize);
            data.lParam as usize
        };
        (index < self.nodes.borrow().len()).then_some(index)
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
        let mut item = send(self.hwnd, TVM_GETNEXTITEM, TVGN_CHILD as usize, parent as isize) as HTREEITEM;
        while item != 0 {
            items.push(item);
            item = send(self.hwnd, TVM_GETNEXTITEM, TVGN_NEXT as usize, item as isize) as HTREEITEM;
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
                send(self.hwnd, TVM_DELETEITEM, 0, *item as isize);
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
            ScreenToClient(self.hwnd, &mut point);
            let mut hit: TVHITTESTINFO = std::mem::zeroed();
            hit.pt = point;
            send(self.hwnd, TVM_HITTEST, 0, &mut hit as *mut _ as isize) as HTREEITEM
        }
    }

    fn selected(&self) -> Option<usize> {
        self.node_of(send(self.hwnd, TVM_GETNEXTITEM, TVGN_CARET as usize, 0) as HTREEITEM)
    }

    /// Gestisce le notifiche della TreeView; il valore restituito va al chiamante di WM_NOTIFY.
    pub fn on_notify(&self, owner: HWND, header: &NMHDR, lparam: LPARAM) -> (isize, Option<Action>) {
        match header.code {
            TVN_ITEMEXPANDINGW => {
                // SAFETY: per questa notifica lParam punta a NMTREEVIEWW.
                let view = unsafe { &*(lparam as *const NMTREEVIEWW) };
                if let Some(index) = self.node_of(view.itemNew.hItem) {
                    self.load(index);
                }
                (0, None)
            }
            TVN_SELCHANGEDW => {
                // SAFETY: per questa notifica lParam punta a NMTREEVIEWW.
                let view = unsafe { &*(lparam as *const NMTREEVIEWW) };
                let open = view.action == TVC_BYMOUSE;
                (0, open.then(|| self.file_at(view.itemNew.hItem)).flatten().map(Action::Open))
            }
            NM_RETURN => (0, self.selected().and_then(|index| self.file_at(self.nodes.borrow()[index].item)).map(Action::Open)),
            NM_RCLICK => {
                let item = self.item_at_cursor();
                if item != 0 {
                    send(self.hwnd, TVM_SELECTITEM, TVGN_CARET as usize, item as isize);
                }
                (1, self.context_menu(owner, self.node_of(item)))
            }
            TVN_BEGINLABELEDITW => (0, None),
            TVN_ENDLABELEDITW => {
                // SAFETY: per questa notifica lParam punta a NMTVDISPINFOW; pszText è null se la modifica è annullata.
                let info = unsafe { &*(lparam as *const NMTVDISPINFOW) };
                if info.item.pszText.is_null() {
                    return (0, None);
                }
                // SAFETY: testo terminato da zero fornito dal controllo.
                let name = unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(info.item.pszText, (0..).take_while(|&i| *info.item.pszText.add(i) != 0).count())) };
                let Some(index) = self.node_of(info.item.hItem) else { return (0, None) };
                let from = self.nodes.borrow()[index].path.clone();
                match fs::rename_path(from.to_string_lossy().into_owned(), name) {
                    Ok(to) => {
                        self.refresh_parent(&from);
                        (0, Some(Action::Renamed { from, to: PathBuf::from(to) }))
                    }
                    Err(error) => {
                        message_box(owner, &error, MB_ICONERROR);
                        (0, None)
                    }
                }
            }
            _ => (0, None),
        }
    }

    fn file_at(&self, item: HTREEITEM) -> Option<PathBuf> {
        let index = self.node_of(item)?;
        let node = &self.nodes.borrow()[index];
        (!node.is_dir).then(|| node.path.clone())
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

    fn context_menu(&self, owner: HWND, node: Option<usize>) -> Option<Action> {
        let entries: &[(usize, &str)] = if node.is_some() {
            &[(MENU_NEW_FILE, "Nuovo file"), (MENU_NEW_FOLDER, "Nuova cartella"), (0, ""), (MENU_RENAME, "Rinomina\tF2"), (MENU_DELETE, "Elimina\tCanc"), (0, ""), (MENU_REVEAL, "Mostra in Esplora risorse"), (MENU_COPY_PATH, "Copia percorso")]
        } else {
            &[(MENU_NEW_FILE, "Nuovo file"), (MENU_NEW_FOLDER, "Nuova cartella"), (0, ""), (MENU_REVEAL, "Mostra in Esplora risorse")]
        };
        let command = popup_menu(owner, entries);
        let path = node.map(|index| self.nodes.borrow()[index].path.clone()).or_else(|| self.root())?;
        match command {
            MENU_NEW_FILE | MENU_NEW_FOLDER => self.create_entry(owner, node, command == MENU_NEW_FOLDER),
            MENU_RENAME => {
                let item = self.nodes.borrow()[node?].item;
                send(self.hwnd, TVM_EDITLABELW, 0, item as isize);
                None
            }
            MENU_DELETE => self.delete(owner, &path),
            MENU_REVEAL => {
                let _ = fs::reveal_in_os(path.to_string_lossy().into_owned());
                None
            }
            MENU_COPY_PATH => {
                copy_to_clipboard(owner, &path.to_string_lossy());
                None
            }
            _ => None,
        }
    }

    pub fn delete_selected(&self, owner: HWND) -> Option<Action> {
        let path = self.nodes.borrow()[self.selected()?].path.clone();
        self.delete(owner, &path)
    }

    pub fn rename_selected(&self) {
        if let Some(index) = self.selected() {
            send(self.hwnd, TVM_EDITLABELW, 0, self.nodes.borrow()[index].item as isize);
        }
    }

    fn delete(&self, owner: HWND, path: &Path) -> Option<Action> {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned());
        if message_box(owner, &format!("Spostare \"{name}\" nel Cestino?"), MB_YESNO | MB_ICONQUESTION) != IDYES {
            return None;
        }
        if let Err(error) = fs::delete_to_trash(path.to_string_lossy().into_owned()) {
            message_box(owner, &error, MB_ICONERROR);
            return None;
        }
        self.refresh_parent(path);
        Some(Action::Deleted(path.to_path_buf()))
    }

    /// Crea con un nome libero e lascia subito rinominare, come in IntelliJ.
    fn create_entry(&self, owner: HWND, node: Option<usize>, folder: bool) -> Option<Action> {
        let dir = self.target_dir(node)?;
        if let Some(index) = node.filter(|&index| self.nodes.borrow()[index].is_dir) {
            self.load(index);
            send(self.hwnd, TVM_EXPAND, TVE_EXPAND as usize, self.nodes.borrow()[index].item as isize);
        }
        let base = if folder { "nuova-cartella" } else { "nuovo-file.txt" };
        let name = (1..).map(|n| numbered(base, n)).find(|name| !dir.join(name).exists())?;
        let created = if folder { fs::create_dir(dir.to_string_lossy().into_owned(), name) } else { fs::create_file(dir.to_string_lossy().into_owned(), name) };
        let path = match created {
            Ok(path) => PathBuf::from(path),
            Err(error) => {
                message_box(owner, &error, MB_ICONERROR);
                return None;
            }
        };
        self.refresh_dir(&dir);
        let item = self.nodes.borrow().iter().find(|node| node.path == path && node.item != 0).map(|node| node.item);
        if let Some(item) = item {
            send(self.hwnd, TVM_SELECTITEM, TVGN_CARET as usize, item as isize);
            send(self.hwnd, TVM_EDITLABELW, 0, item as isize);
        }
        (!folder).then_some(Action::Open(path))
    }
}

/// `nuovo-file.txt`, `nuovo-file-2.txt`, …; `nuova-cartella`, `nuova-cartella-2`, …
fn numbered(base: &str, n: usize) -> String {
    if n == 1 {
        return base.to_string();
    }
    match base.split_once('.') {
        Some((stem, extension)) => format!("{stem}-{n}.{extension}"),
        None => format!("{base}-{n}"),
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

#[cfg(test)]
mod tests {
    use super::numbered;

    #[test]
    fn numbers_new_names_before_the_extension() {
        assert_eq!(numbered("nuovo-file.txt", 1), "nuovo-file.txt");
        assert_eq!(numbered("nuovo-file.txt", 2), "nuovo-file-2.txt");
        assert_eq!(numbered("nuova-cartella", 3), "nuova-cartella-3");
    }
}

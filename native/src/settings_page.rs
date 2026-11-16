//! Pagina impostazioni (Ctrl+Alt+S) come quella della versione Tauri: copre tutta la finestra, a sinistra le sezioni,
//! a destra il titolo con ✕. Editor e Plugin come su Tauri, più "Git e terminale" (metodo di update e shell), che qui
//! non hanno altri menu. Le modifiche valgono alla chiusura (✕ o Esc).
use std::cell::RefCell;

use steroitor_core::shells::Shell;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, GetKeyState, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_TAB};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::find::window_text;
use crate::paint::{fill, frame, text, text_width};
use crate::plugins::PLUGINS;
use crate::session::{EditorStyle, Settings};
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorSettings";
const NAV: i32 = 200;
const NAV_ITEM: i32 = 30;
const GUTTER: i32 = 24;
const FORM: i32 = 420;
const FIELD: i32 = 80;
const ROW: i32 = 34;
const CARD: i32 = 720;
const CONTENT_TOP: i32 = 52;
const EN_SETFOCUS: u32 = 0x0100;
const EN_KILLFOCUS: u32 = 0x0200;
const FOLLOW_TIMER: usize = 1;

/// Campi numerici dell'editor: etichetta, minimo, massimo (gli stessi della versione Tauri).
const FIELDS: [(&str, f64, f64); 4] = [("Dimensione carattere (px)", 8.0, 32.0), ("Interlinea", 1.0, 2.5), ("Larghezza tab", 1.0, 16.0), ("Indentazione (spazi)", 1.0, 16.0)];

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Editor,
    Plugins,
    Git,
}

const SECTIONS: [(Section, &str); 3] = [(Section::Editor, "Editor"), (Section::Plugins, "Plugin"), (Section::Git, "Git e terminale")];

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Section(Section),
    Close,
    UseTabs,
    Rebase(bool),
    Shell(usize),
    Plugin(usize),
    PluginOption(usize, usize),
}

struct Page {
    owner: HWND,
    palette: Palette,
    brush: HBRUSH,
    title_font: HFONT,
    section: Section,
    edits: [HWND; 4],
    use_tabs: bool,
    rebase: bool,
    shells: Vec<Shell>,
    shell: usize,
    /// Per ogni plugin di PLUGINS: attivo e le sue opzioni, nell'ordine di `options`.
    plugins: Vec<(bool, Vec<bool>)>,
    /// Scorrimento verticale del contenuto e sua altezza all'ultimo disegno.
    scroll: i32,
    content: i32,
    hits: Vec<(RECT, Hit)>,
    done: bool,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

fn values(style: &EditorStyle) -> [f64; 4] {
    [style.font_size, style.line_height, style.tab_size as f64, style.indent_size as f64]
}

/// Valore del campo limitato all'intervallo; se non è un numero resta quello di prima.
fn parse_field(raw: &str, index: usize, previous: f64) -> f64 {
    let (_, min, max) = FIELDS[index];
    raw.trim().replace(',', ".").parse::<f64>().ok().filter(|value| value.is_finite()).map_or(previous, |value| value.clamp(min, max))
}

fn format_field(value: f64) -> String {
    if value.fract() == 0.0 { format!("{value:.0}") } else { format!("{value}") }
}

fn scaled(hwnd: HWND, value: i32) -> i32 {
    // SAFETY: lettura dei DPI di una finestra valida.
    value * unsafe { GetDpiForWindow(hwnd) } as i32 / 96
}

/// Testo a capo dentro `rect` (solo la larghezza conta); con `draw` falso misura soltanto. Restituisce l'altezza.
unsafe fn wrapped(dc: HDC, value: &str, rect: &RECT, color: u32, draw: bool) -> i32 {
    let mut chars: Vec<u16> = value.encode_utf16().collect();
    let mut measured = *rect;
    DrawTextW(dc, chars.as_mut_ptr(), chars.len() as i32, &mut measured, DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX);
    if draw {
        SetTextColor(dc, color);
        let mut area = RECT { bottom: rect.top + measured.bottom - measured.top, ..*rect };
        DrawTextW(dc, chars.as_mut_ptr(), chars.len() as i32, &mut area, DT_WORDBREAK | DT_NOPREFIX);
    }
    measured.bottom - measured.top
}

/// Casella di spunta con etichetta a partire da `left`, centrata nella riga; restituisce l'area cliccabile.
unsafe fn checkbox(dc: HDC, palette: &Palette, left: i32, row: &RECT, label: &str, on: bool, color: u32) -> RECT {
    let size = (row.bottom - row.top) * 13 / 24;
    let middle = (row.top + row.bottom) / 2;
    let box_rect = RECT { left, top: middle - size / 2, right: left + size, bottom: middle + size / 2 };
    frame(dc, &box_rect, palette.muted);
    if on {
        fill(dc, &RECT { left: box_rect.left + 3, top: box_rect.top + 3, right: box_rect.right - 3, bottom: box_rect.bottom - 3 }, palette.accent);
    }
    let label_left = box_rect.right + size / 2;
    text(dc, label, &RECT { left: label_left, ..*row }, color, DT_LEFT);
    RECT { left, right: label_left + text_width(dc, label), ..*row }
}

/// Scheda di un plugin da `top`; con `draw` falso misura soltanto. Restituisce dove finisce.
unsafe fn plugin_card(dc: HDC, hwnd: HWND, page: &Page, index: usize, area: &RECT, draw: bool, hits: &mut Vec<(RECT, Hit)>) -> i32 {
    let s = |value: i32| scaled(hwnd, value);
    let palette = &page.palette;
    let plugin = &PLUGINS[index];
    let (enabled, options) = &page.plugins[index];
    let inner = RECT { left: area.left + s(12), top: area.top + s(10), right: area.right - s(12), bottom: area.bottom };
    let line = |top: i32| RECT { top, bottom: top + s(22), ..inner };
    let mut top = inner.top;
    if draw {
        let previous = SelectObject(dc, page.title_font);
        text(dc, plugin.name, &line(top), palette.fg, DT_LEFT);
        let name_width = text_width(dc, plugin.name);
        SelectObject(dc, previous);
        text(dc, &format!("{} · Steroitor", plugin.version), &RECT { left: inner.left + name_width + s(8), ..line(top) }, palette.muted, DT_LEFT);
        let label = "Attivo";
        let toggle = line(top);
        let left = toggle.right - text_width(dc, label) - s(20);
        hits.push((checkbox(dc, palette, left, &toggle, label, *enabled, palette.fg), Hit::Plugin(index)));
    }
    top += s(28);
    top += wrapped(dc, plugin.description, &line(top), palette.fg, draw) + s(8);
    let mut left = inner.left;
    for (name, extensions) in plugin.languages {
        let extensions = extensions.iter().map(|extension| format!(".{extension}")).collect::<Vec<_>>().join(" ");
        let width = text_width(dc, name) + s(4) + text_width(dc, &extensions) + s(14);
        if left > inner.left && left + width > inner.right {
            left = inner.left;
            top += s(26);
        }
        if draw {
            let chip = RECT { left, top, right: left + width, bottom: top + s(20) };
            frame(dc, &chip, palette.border);
            text(dc, name, &RECT { left: left + s(7), ..chip }, palette.fg, DT_LEFT);
            text(dc, &extensions, &RECT { left: left + s(7) + text_width(dc, name) + s(4), ..chip }, palette.muted, DT_LEFT);
        }
        left += width + s(4);
    }
    top += s(30);
    for (option, (_, label, description)) in plugin.options.iter().enumerate() {
        if draw {
            let area = checkbox(dc, palette, inner.left, &line(top), label, options[option], if *enabled { palette.fg } else { palette.muted });
            if *enabled {
                hits.push((area, Hit::PluginOption(index, option)));
            }
        }
        top += s(24);
        top += wrapped(dc, description, &RECT { left: inner.left + s(22), ..line(top) }, palette.muted, draw) + s(8);
    }
    top += wrapped(dc, &format!("Richiede {}.", plugin.requires), &line(top), palette.muted, draw);
    top + s(10)
}

unsafe fn paint(hwnd: HWND, page: &mut Page) {
    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let target = BeginPaint(hwnd, &mut paint);
    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(hwnd, &mut client);
    let dc = CreateCompatibleDC(target);
    let bitmap = CreateCompatibleBitmap(target, client.right, client.bottom);
    let old_bitmap = SelectObject(dc, bitmap);
    let s = |value: i32| scaled(hwnd, value);
    let palette = page.palette;
    let previous = SelectObject(dc, SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT);
    SetBkMode(dc, TRANSPARENT as i32);
    let mut hits = Vec::new();

    let main = RECT { left: s(NAV) + s(GUTTER), top: 0, right: client.right - s(GUTTER), bottom: client.bottom };
    fill(dc, &client, palette.editor);
    let mut top = s(CONTENT_TOP) - page.scroll;
    let row = |top: i32| RECT { top, bottom: top + s(26), ..main };
    match page.section {
        Section::Editor => {
            for (index, (label, _, _)) in FIELDS.iter().enumerate() {
                text(dc, label, &row(top + index as i32 * s(ROW)), palette.fg, DT_LEFT);
                let field = RECT { left: main.left + s(FORM - FIELD), top: top + index as i32 * s(ROW), right: main.left + s(FORM), bottom: top + index as i32 * s(ROW) + s(26) };
                fill(dc, &field, palette.editor);
                frame(dc, &field, if GetFocus() == page.edits[index] { palette.accent } else { palette.border });
            }
            let check_row = row(top + 4 * s(ROW));
            hits.push((checkbox(dc, &palette, main.left, &check_row, "Indenta con il carattere tab", page.use_tabs, palette.fg), Hit::UseTabs));
            top += 5 * s(ROW);
        }
        Section::Git => {
            let pill = |dc: HDC, left: i32, top: i32, label: &str, on: bool, hit: Hit, hits: &mut Vec<(RECT, Hit)>| -> i32 {
                let area = RECT { left, top, right: left + text_width(dc, label) + s(20), bottom: top + s(26) };
                if on {
                    fill(dc, &area, palette.accent);
                    text(dc, label, &area, 0xFFFFFF, DT_CENTER);
                } else {
                    frame(dc, &area, palette.border);
                    text(dc, label, &area, palette.fg, DT_CENTER);
                }
                hits.push((area, hit));
                area.right + s(6)
            };
            text(dc, "Update (pull)", &row(top), palette.fg, DT_LEFT);
            let left = pill(dc, main.left + s(FORM / 2), top, "Merge", !page.rebase, Hit::Rebase(false), &mut hits);
            pill(dc, left, top, "Rebase", page.rebase, Hit::Rebase(true), &mut hits);
            top += s(ROW + 8);
            text(dc, "Shell dei nuovi terminali", &row(top), palette.fg, DT_LEFT);
            top += s(ROW);
            let mut left = main.left;
            for (index, shell) in page.shells.iter().enumerate() {
                if left > main.left && left + text_width(dc, &shell.name) + s(20) > main.right {
                    left = main.left;
                    top += s(32);
                }
                left = pill(dc, left, top, &shell.name, index == page.shell, Hit::Shell(index), &mut hits);
            }
            top += s(ROW);
        }
        Section::Plugins => {
            let note = RECT { right: main.left + s(CARD), ..row(top) };
            top += wrapped(dc, "Plugin e opzioni si applicano alla chiusura della pagina, a tutti i file aperti.", &note, palette.muted, true) + s(12);
            // Schede affiancate finché ci stanno; quelle di una riga hanno l'altezza della più alta.
            let gap = s(8);
            let width = (main.right - main.left).min(s(CARD));
            let columns = ((main.right - main.left + gap) / (width + gap)).max(1) as usize;
            let indices: Vec<usize> = (0..PLUGINS.len()).collect();
            for row in indices.chunks(columns) {
                let slot = |column: usize| {
                    let left = main.left + column as i32 * (width + gap);
                    RECT { left, top, right: left + width, bottom: top }
                };
                let bottom = row.iter().enumerate().map(|(column, &index)| plugin_card(dc, hwnd, page, index, &slot(column), false, &mut hits)).max().unwrap_or(top);
                for (column, &index) in row.iter().enumerate() {
                    let card = RECT { bottom, ..slot(column) };
                    fill(dc, &card, palette.panel);
                    frame(dc, &card, palette.border);
                    plugin_card(dc, hwnd, page, index, &card, true, &mut hits);
                }
                top = bottom + gap;
            }
        }
    }
    page.content = top + page.scroll + s(GUTTER);

    // Intestazione e navigazione sopra il contenuto che scorre.
    fill(dc, &RECT { bottom: s(CONTENT_TOP) - s(8), ..main }, palette.editor);
    let label = SECTIONS.iter().find(|(section, _)| *section == page.section).map_or("", |(_, label)| label);
    let title_font = SelectObject(dc, page.title_font);
    text(dc, label, &RECT { top: s(12), bottom: s(40), ..main }, palette.fg, DT_LEFT);
    SelectObject(dc, title_font);
    let close = RECT { left: main.right - s(28), top: s(12), right: main.right, bottom: s(40) };
    text(dc, "✕", &close, palette.muted, DT_CENTER);
    hits.push((close, Hit::Close));

    let nav = RECT { left: 0, top: 0, right: s(NAV), bottom: client.bottom };
    fill(dc, &nav, palette.panel);
    fill(dc, &RECT { left: nav.right - 1, ..nav }, palette.border);
    let title_font = SelectObject(dc, page.title_font);
    text(dc, "Impostazioni", &RECT { left: s(16), top: s(12), right: nav.right - s(8), bottom: s(36) }, palette.fg, DT_LEFT);
    SelectObject(dc, title_font);
    let mut item_top = s(48);
    for (section, label) in SECTIONS {
        let item = RECT { left: s(8), top: item_top, right: nav.right - s(8), bottom: item_top + s(NAV_ITEM) };
        let on = section == page.section;
        if on {
            fill(dc, &item, palette.hover);
        }
        text(dc, label, &RECT { left: item.left + s(8), ..item }, if on { palette.accent } else { palette.fg }, DT_LEFT);
        hits.push((item, Hit::Section(section)));
        item_top += s(NAV_ITEM + 2);
    }

    page.hits = hits;
    BitBlt(target, 0, 0, client.right, client.bottom, dc, 0, 0, SRCCOPY);
    SelectObject(dc, previous);
    SelectObject(dc, old_bitmap);
    DeleteObject(bitmap);
    DeleteDC(dc);
    EndPaint(hwnd, &paint);
}

/// I campi esistono solo nella sezione Editor, al loro posto sopra i riquadri disegnati.
unsafe fn place_edits(hwnd: HWND, page: &Page) {
    let s = |value: i32| scaled(hwnd, value);
    let visible = page.section == Section::Editor;
    for (index, &edit) in page.edits.iter().enumerate() {
        let top = s(CONTENT_TOP) + index as i32 * s(ROW);
        let left = s(NAV) + s(GUTTER) + s(FORM - FIELD);
        SetWindowPos(edit, std::ptr::null_mut(), left + s(6), top + s(5), s(FIELD) - s(12), s(16), SWP_NOZORDER);
        ShowWindow(edit, if visible { SW_SHOW } else { SW_HIDE });
    }
}

unsafe extern "system" fn page_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_ERASEBKGND {
        return 1;
    }
    let handled = PAGE.with(|page| {
        let mut page = page.try_borrow_mut().ok()?;
        let page = page.as_mut()?;
        match message {
            WM_PAINT => paint(hwnd, page),
            // Il timer gira anche dentro il ciclo di spostamento della finestra principale: la pagina la segue.
            WM_TIMER if wparam == FOLLOW_TIMER => follow_owner(hwnd, page.owner),
            WM_CTLCOLOREDIT => {
                SetTextColor(wparam as HDC, page.palette.fg);
                SetBkColor(wparam as HDC, page.palette.editor);
                return Some(page.brush as LRESULT);
            }
            WM_COMMAND if matches!(((wparam >> 16) & 0xFFFF) as u32, EN_SETFOCUS | EN_KILLFOCUS) => {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            WM_MOUSEWHEEL if page.section != Section::Editor => {
                let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                GetClientRect(hwnd, &mut client);
                let delta = ((wparam >> 16) & 0xFFFF) as i16 as i32;
                let limit = (page.content - client.bottom).max(0);
                page.scroll = (page.scroll - delta * scaled(hwnd, 60) / 120).clamp(0, limit);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            WM_LBUTTONDOWN => {
                let (x, y) = ((lparam & 0xFFFF) as i16 as i32, ((lparam >> 16) & 0xFFFF) as i16 as i32);
                let hit = page.hits.iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, hit)| *hit);
                match hit {
                    Some(Hit::Section(section)) => {
                        page.section = section;
                        page.scroll = 0;
                        place_edits(hwnd, page);
                    }
                    Some(Hit::Close) => page.done = true,
                    Some(Hit::UseTabs) => page.use_tabs = !page.use_tabs,
                    Some(Hit::Rebase(rebase)) => page.rebase = rebase,
                    Some(Hit::Shell(index)) => page.shell = index,
                    Some(Hit::Plugin(index)) => page.plugins[index].0 = !page.plugins[index].0,
                    Some(Hit::PluginOption(index, option)) => page.plugins[index].1[option] = !page.plugins[index].1[option],
                    None => return Some(0),
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            WM_CLOSE => page.done = true,
            _ => return None,
        }
        Some(0)
    });
    handled.unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
}

/// Riporta la pagina sull'area client della finestra principale, se questa si è spostata o ridimensionata.
unsafe fn follow_owner(hwnd: HWND, owner: HWND) {
    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(owner, &mut client);
    let mut origin = POINT { x: 0, y: 0 };
    ClientToScreen(owner, &mut origin);
    let mut current = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetWindowRect(hwnd, &mut current);
    if (current.left, current.top, current.right - current.left, current.bottom - current.top) != (origin.x, origin.y, client.right, client.bottom) && client.right > 0 {
        SetWindowPos(hwnd, std::ptr::null_mut(), origin.x, origin.y, client.right, client.bottom, SWP_NOZORDER | SWP_NOACTIVATE);
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
}

/// Mostra la pagina sopra tutta la finestra; alla chiusura le impostazioni come le ha lasciate l'utente.
pub fn edit(owner: HWND, palette: &Palette, font: HFONT, current: &Settings, shells: Vec<Shell>) -> Option<Settings> {
    let class_name = wide(CLASS_NAME);
    let shell = shells.iter().position(|shell| shell.id == current.terminal_shell).unwrap_or(0);
    // SAFETY: finestra modale creata, usata e distrutta qui; il proprietario è disabilitato per tutta la durata.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSEXW { cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(page_proc), hInstance: instance, hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW), lpszClassName: class_name.as_ptr(), ..std::mem::zeroed() };
        RegisterClassExW(&class);
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(owner, &mut client);
        let mut origin = POINT { x: 0, y: 0 };
        ClientToScreen(owner, &mut origin);
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), std::ptr::null(), WS_POPUP | WS_CLIPCHILDREN, origin.x, origin.y, client.right, client.bottom, owner, std::ptr::null_mut(), instance, std::ptr::null());
        SendMessageW(hwnd, WM_SETFONT, font as usize, 0);
        let face = wide("Segoe UI");
        let title_font = CreateFontW(-scaled(owner, 15), 0, 0, 0, 600, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, face.as_ptr());
        let edit_class = wide("EDIT");
        let initial = values(&current.editor);
        let edits = [0, 1, 2, 3].map(|index| {
            let value = wide(&format_field(initial[index]));
            let edit = CreateWindowExW(0, edit_class.as_ptr(), value.as_ptr(), WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL as u32, 0, 0, 0, 0, hwnd, std::ptr::null_mut(), instance, std::ptr::null());
            SendMessageW(edit, WM_SETFONT, font as usize, 0);
            edit
        });
        let page = Page {
            owner,
            palette: *palette,
            brush: CreateSolidBrush(palette.editor),
            title_font,
            section: Section::Editor,
            edits,
            use_tabs: current.editor.use_tabs,
            rebase: current.pull_rebase(),
            shells,
            shell,
            plugins: PLUGINS.iter().map(|plugin| (current.plugin_enabled(plugin.id), plugin.options.iter().map(|(option, _, _)| plugin.option_on(current, option)).collect())).collect(),
            scroll: 0,
            content: 0,
            hits: Vec::new(),
            done: false,
        };
        place_edits(hwnd, &page);
        PAGE.with(|slot| *slot.borrow_mut() = Some(page));

        // La finestra principale resta attiva per spostarla e ridimensionarla dalla cornice; la pagina copre il resto.
        SetTimer(hwnd, FOLLOW_TIMER, 30, None);
        // Chiudere l'app con la pagina aperta non è previsto: la ✕ della finestra torna attiva alla chiusura della pagina.
        EnableMenuItem(GetSystemMenu(owner, 0), SC_CLOSE, MF_BYCOMMAND | MF_GRAYED);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(edits[0]);
        SendMessageW(edits[0], 0x00B1, 0, -1);
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if PAGE.with(|page| page.borrow().as_ref().is_some_and(|page| page.done)) {
                break;
            }
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                // WM_QUIT arrivato durante il dialogo: va rimesso in coda per il ciclo principale.
                PostQuitMessage(0);
                break;
            }
            let ours = msg.hwnd == hwnd || IsChild(hwnd, msg.hwnd) != 0;
            if !ours && matches!(msg.message, WM_KEYDOWN | WM_KEYUP | WM_CHAR) {
                // Tasti diretti all'editor sotto la pagina (dopo un clic sulla cornice): tornano alla pagina.
                SetFocus(edits[0]);
                continue;
            }
            if msg.message == WM_KEYDOWN {
                match msg.wParam as u16 {
                    VK_ESCAPE => break,
                    VK_RETURN => continue,
                    VK_TAB if edits.contains(&GetFocus()) => {
                        let backwards = GetKeyState(VK_SHIFT as i32) < 0;
                        let position = edits.iter().position(|&edit| edit == GetFocus()).unwrap_or(0);
                        let next = edits[(position + if backwards { 3 } else { 1 }) % 4];
                        SetFocus(next);
                        SendMessageW(next, 0x00B1, 0, -1);
                        continue;
                    }
                    _ => {}
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let page = PAGE.with(|page| page.borrow_mut().take());
        let raw: Vec<String> = edits.iter().map(|&edit| window_text(edit)).collect();
        KillTimer(hwnd, FOLLOW_TIMER);
        EnableMenuItem(GetSystemMenu(owner, 0), SC_CLOSE, MF_BYCOMMAND | MF_ENABLED);
        DestroyWindow(hwnd);
        DeleteObject(title_font);
        let page = page?;
        DeleteObject(page.brush);
        SetFocus(owner);
        let fields: Vec<f64> = (0..4).map(|index| parse_field(&raw[index], index, initial[index])).collect();
        let editor = EditorStyle { font_size: fields[0], line_height: fields[1], tab_size: fields[2] as usize, indent_size: fields[3] as usize, use_tabs: page.use_tabs };
        let terminal_shell = page.shells.get(page.shell).map_or_else(String::new, |shell| shell.id.clone());
        let mut disabled_plugins: Vec<String> = current.disabled_plugins.iter().filter(|id| !PLUGINS.iter().any(|plugin| plugin.id == id.as_str())).cloned().collect();
        let mut plugin_options = current.plugin_options.clone();
        for (plugin, (enabled, options)) in PLUGINS.iter().zip(&page.plugins) {
            if !enabled {
                disabled_plugins.push(plugin.id.into());
            }
            let saved = plugin_options.entry(plugin.id.into()).or_default();
            for ((option, _, _), on) in plugin.options.iter().zip(options) {
                saved.insert((*option).into(), *on);
            }
        }
        Some(Settings { editor, update_method: (if page.rebase { "rebase" } else { "merge" }).into(), terminal_shell, warn_crlf: current.warn_crlf, disabled_plugins, plugin_options, clone_parent: current.clone_parent.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_and_keeps_invalid_fields() {
        assert_eq!(parse_field("1,8", 1, 1.5), 1.8);
        assert_eq!(parse_field("99", 0, 14.0), 32.0);
        assert_eq!(parse_field("abc", 2, 4.0), 4.0);
        assert_eq!(format_field(14.0), "14");
        assert_eq!(format_field(1.5), "1.5");
    }
}

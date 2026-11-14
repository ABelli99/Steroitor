//! Pagina impostazioni, modale (Ctrl+Alt+S): Editor come nella versione Tauri, più metodo di update e shell del
//! terminale, che qui non hanno altri menu. Invio salva, Esc annulla.
use std::cell::RefCell;

use steroitor_core::shells::Shell;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, GetKeyState, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_TAB};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::dialogs::wide;
use crate::find::window_text;
use crate::paint::{fill, frame, text, text_width};
use crate::plugins::PLUGINS;
use crate::session::{EditorStyle, Settings};
use crate::theme::Palette;

const CLASS_NAME: &str = "SteroitorSettings";
const WIDTH: i32 = 480;
const HEIGHT: i32 = 710;
const FIELD_LEFT: i32 = 260;
const ROW: i32 = 34;

/// Campi numerici dell'editor: etichetta, minimo, massimo (gli stessi della versione Tauri).
const FIELDS: [(&str, f64, f64); 4] = [("Dimensione carattere (px)", 8.0, 32.0), ("Interlinea", 1.0, 2.5), ("Larghezza tab", 1.0, 16.0), ("Indentazione (spazi)", 1.0, 16.0)];

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    UseTabs,
    Rebase(bool),
    Shell(usize),
    Plugin(usize),
    PluginOption(usize, usize),
    Save,
    Cancel,
}

struct Page {
    palette: Palette,
    brush: HBRUSH,
    use_tabs: bool,
    rebase: bool,
    shells: Vec<Shell>,
    shell: usize,
    /// Per ogni plugin di PLUGINS: attivo e le sue opzioni, nell'ordine di `options`.
    plugins: Vec<(bool, Vec<bool>)>,
    hits: Vec<(RECT, Hit)>,
    done: Option<bool>,
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

/// Casella di spunta con etichetta a partire da `left`, centrata nella riga; restituisce l'area cliccabile.
unsafe fn checkbox(dc: HDC, palette: &Palette, left: i32, row: &RECT, label: &str, on: bool, color: u32) -> RECT {
    let size = (row.bottom - row.top) * 14 / 26;
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

unsafe fn paint(hwnd: HWND, page: &mut Page) {
    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let dc = BeginPaint(hwnd, &mut paint);
    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(hwnd, &mut client);
    let dpi = GetDpiForWindow(hwnd) as i32;
    let s = |value: i32| value * dpi / 96;
    let palette = page.palette;
    fill(dc, &client, palette.panel);
    let previous = SelectObject(dc, SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT);
    SetBkMode(dc, TRANSPARENT as i32);
    let row = |top: i32| RECT { left: s(16), top: s(top), right: client.right - s(16), bottom: s(top + 26) };
    let mut hits = Vec::new();

    text(dc, "Editor", &row(12), palette.accent, DT_LEFT);
    for (index, (label, _, _)) in FIELDS.iter().enumerate() {
        text(dc, label, &RECT { right: s(FIELD_LEFT - 8), ..row(44 + index as i32 * ROW) }, palette.fg, DT_LEFT);
    }
    let check_row = row(44 + 4 * ROW);
    hits.push((checkbox(dc, &palette, check_row.left, &check_row, "Indenta con il carattere tab", page.use_tabs, palette.fg), Hit::UseTabs));

    let git_top = 44 + 5 * ROW + 12;
    text(dc, "Git e terminale", &row(git_top), palette.accent, DT_LEFT);
    text(dc, "Update (pull)", &RECT { right: s(FIELD_LEFT - 8), ..row(git_top + 32) }, palette.fg, DT_LEFT);
    let pill = |dc: HDC, left: i32, top: i32, label: &str, on: bool, hit: Hit, hits: &mut Vec<(RECT, Hit)>| -> i32 {
        let area = RECT { left, top: s(top), right: left + text_width(dc, label) + s(20), bottom: s(top + 26) };
        if on {
            fill(dc, &area, palette.accent);
            text(dc, label, &area, palette.editor, DT_CENTER);
        } else {
            frame(dc, &area, palette.border);
            text(dc, label, &area, palette.fg, DT_CENTER);
        }
        hits.push((area, hit));
        area.right + s(6)
    };
    let left = pill(dc, s(FIELD_LEFT), git_top + 32, "Merge", !page.rebase, Hit::Rebase(false), &mut hits);
    pill(dc, left, git_top + 32, "Rebase", page.rebase, Hit::Rebase(true), &mut hits);
    text(dc, "Shell dei nuovi terminali", &row(git_top + 32 + ROW), palette.fg, DT_LEFT);
    let (mut left, mut top) = (s(16), git_top + 32 + 2 * ROW - 4);
    for (index, shell) in page.shells.iter().enumerate() {
        if left + text_width(dc, &shell.name) + s(20) > client.right - s(16) {
            left = s(16);
            top += 32;
        }
        left = pill(dc, left, top, &shell.name, index == page.shell, Hit::Shell(index), &mut hits);
    }

    let mut top = top + 44;
    text(dc, "Plugin", &row(top), palette.accent, DT_LEFT);
    top += 30;
    for (index, plugin) in PLUGINS.iter().enumerate() {
        let (enabled, options) = page.plugins[index].clone();
        let name_row = row(top);
        hits.push((checkbox(dc, &palette, name_row.left, &name_row, plugin.name, enabled, palette.fg), Hit::Plugin(index)));
        text(dc, plugin.requires, &RECT { left: s(40), ..row(top + 22) }, palette.muted, DT_LEFT | DT_END_ELLIPSIS);
        let mut option_top = top + 46;
        let mut left = s(40);
        for (option, (_, label)) in plugin.options.iter().enumerate() {
            // Le opzioni che non stanno nella riga vanno a capo.
            if left > s(40) && left + s(21) + text_width(dc, label) > client.right - s(16) {
                left = s(40);
                option_top += 28;
            }
            let area = checkbox(dc, &palette, left, &row(option_top), label, options[option], if enabled { palette.fg } else { palette.muted });
            hits.push((area, Hit::PluginOption(index, option)));
            left = area.right + s(18);
        }
        top = option_top + 36;
    }

    let buttons = RECT { top: client.bottom - s(44), bottom: client.bottom - s(14), ..client };
    text(dc, "Invio salva, Esc annulla", &RECT { left: s(16), ..buttons }, palette.muted, DT_LEFT);
    let mut right = client.right - s(16);
    for (label, hit) in [("Annulla", Hit::Cancel), ("Salva", Hit::Save)] {
        let area = RECT { left: right - text_width(dc, label) - s(28), right, ..buttons };
        if hit == Hit::Save {
            fill(dc, &area, palette.accent);
            text(dc, label, &area, palette.editor, DT_CENTER);
        } else {
            frame(dc, &area, palette.border);
            text(dc, label, &area, palette.fg, DT_CENTER);
        }
        hits.push((area, hit));
        right = area.left - s(8);
    }
    page.hits = hits;
    SelectObject(dc, previous);
    EndPaint(hwnd, &paint);
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
            WM_CTLCOLOREDIT => {
                SetTextColor(wparam as HDC, page.palette.fg);
                SetBkColor(wparam as HDC, page.palette.editor);
                return Some(page.brush as LRESULT);
            }
            WM_LBUTTONDOWN => {
                let (x, y) = ((lparam & 0xFFFF) as i16 as i32, ((lparam >> 16) & 0xFFFF) as i16 as i32);
                let hit = page.hits.iter().find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|(_, hit)| *hit);
                match hit {
                    Some(Hit::UseTabs) => page.use_tabs = !page.use_tabs,
                    Some(Hit::Rebase(rebase)) => page.rebase = rebase,
                    Some(Hit::Shell(index)) => page.shell = index,
                    Some(Hit::Plugin(index)) => page.plugins[index].0 = !page.plugins[index].0,
                    Some(Hit::PluginOption(index, option)) => page.plugins[index].1[option] = !page.plugins[index].1[option],
                    Some(Hit::Save) => page.done = Some(true),
                    Some(Hit::Cancel) => page.done = Some(false),
                    None => return Some(0),
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            WM_CLOSE => page.done = Some(false),
            _ => return None,
        }
        Some(0)
    });
    handled.unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
}

/// Mostra la pagina; Some con le impostazioni modificate se l'utente salva.
pub fn edit(owner: HWND, palette: &Palette, font: HFONT, current: &Settings, shells: Vec<Shell>) -> Option<Settings> {
    let class_name = wide(CLASS_NAME);
    let title = wide("Impostazioni");
    let shell = shells.iter().position(|shell| shell.id == current.terminal_shell).unwrap_or(0);
    // SAFETY: finestra modale creata, usata e distrutta qui; il proprietario è disabilitato per tutta la durata.
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(page_proc),
            hInstance: instance,
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&class);
        let dpi = GetDpiForWindow(owner) as i32;
        let s = |value: i32| value * dpi / 96;
        let mut owner_rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(owner, &mut owner_rect);
        let (width, height) = (s(WIDTH), s(HEIGHT));
        let (left, top) = (owner_rect.left + (owner_rect.right - owner_rect.left - width) / 2, owner_rect.top + (owner_rect.bottom - owner_rect.top - height) / 3);
        let hwnd = CreateWindowExW(WS_EX_DLGMODALFRAME, class_name.as_ptr(), title.as_ptr(), WS_POPUP | WS_CAPTION | WS_SYSMENU, left, top, width, height, owner, std::ptr::null_mut(), instance, std::ptr::null());
        let dark: i32 = palette.dark.into();
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, (&dark as *const i32).cast(), 4);
        SendMessageW(hwnd, WM_SETFONT, font as usize, 0);
        let edit_class = wide("EDIT");
        let initial = values(&current.editor);
        let edits = [0, 1, 2, 3].map(|index| {
            let value = wide(&format_field(initial[index]));
            let edit = CreateWindowExW(0, edit_class.as_ptr(), value.as_ptr(), WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32, s(FIELD_LEFT), s(44 + index as i32 * ROW), s(90), s(26), hwnd, std::ptr::null_mut(), instance, std::ptr::null());
            SendMessageW(edit, WM_SETFONT, font as usize, 0);
            edit
        });
        PAGE.with(|page| {
            *page.borrow_mut() = Some(Page {
                palette: *palette,
                brush: CreateSolidBrush(palette.editor),
                use_tabs: current.editor.use_tabs,
                rebase: current.pull_rebase(),
                shells,
                shell,
                plugins: PLUGINS.iter().map(|plugin| (current.plugin_enabled(plugin.id), plugin.options.iter().map(|(option, _)| current.plugin_option(plugin.id, option)).collect())).collect(),
                hits: Vec::new(),
                done: None,
            })
        });

        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(edits[0]);
        SendMessageW(edits[0], 0x00B1, 0, -1);
        let mut msg: MSG = std::mem::zeroed();
        let saved = loop {
            if let Some(done) = PAGE.with(|page| page.borrow().as_ref().and_then(|page| page.done)) {
                break done;
            }
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                // WM_QUIT arrivato durante il dialogo: va rimesso in coda per il ciclo principale.
                PostQuitMessage(0);
                break false;
            }
            if msg.message == WM_KEYDOWN {
                match msg.wParam as u16 {
                    VK_RETURN => break true,
                    VK_ESCAPE => break false,
                    VK_TAB => {
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
        };
        let page = PAGE.with(|page| page.borrow_mut().take());
        let raw: Vec<String> = edits.iter().map(|&edit| window_text(edit)).collect();
        EnableWindow(owner, 1);
        DestroyWindow(hwnd);
        let page = page?;
        DeleteObject(page.brush);
        SetFocus(owner);
        if !saved {
            return None;
        }
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
            for ((option, _), on) in plugin.options.iter().zip(options) {
                saved.insert((*option).into(), *on);
            }
        }
        Some(Settings { editor, update_method: (if page.rebase { "rebase" } else { "merge" }).into(), terminal_shell, warn_crlf: current.warn_crlf, disabled_plugins, plugin_options })
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

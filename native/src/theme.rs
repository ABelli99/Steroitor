//! Colori dell'app (gli stessi di `src/app.css` della versione Tauri) e tema chiaro/scuro di sistema.
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

/// Colore in formato Win32/Scintilla (0x00BBGGRR) da 0xRRGGBB.
const fn rgb(hex: u32) -> u32 {
    ((hex & 0xFF) << 16) | (hex & 0xFF00) | ((hex >> 16) & 0xFF)
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub panel: u32,
    pub editor: u32,
    pub fg: u32,
    pub muted: u32,
    pub border: u32,
    pub accent: u32,
    pub selection: u32,
    pub active_line: u32,
    pub danger: u32,
    pub keyword: u32,
    pub string: u32,
    pub number: u32,
    pub comment: u32,
    pub function: u32,
    pub kind: u32,
    pub property: u32,
    pub tag: u32,
    pub attribute: u32,
}

const DARK: Palette = Palette {
    dark: true,
    panel: rgb(0x232427),
    editor: rgb(0x1e1f22),
    fg: rgb(0xd4d4d4),
    muted: rgb(0x6f737a),
    border: rgb(0x393b40),
    accent: rgb(0x5a9cf8),
    selection: rgb(0x214283),
    active_line: rgb(0x26272a),
    danger: rgb(0xef5350),
    keyword: rgb(0xcf8e6d),
    string: rgb(0x6aab73),
    number: rgb(0x2aacb8),
    comment: rgb(0x7a7e85),
    function: rgb(0x56a8f5),
    kind: rgb(0xc77dbb),
    property: rgb(0xc77dbb),
    tag: rgb(0xd5b778),
    attribute: rgb(0xbababa),
};

const LIGHT: Palette = Palette {
    dark: false,
    panel: rgb(0xf7f7f7),
    editor: rgb(0xffffff),
    fg: rgb(0x1f1f1f),
    muted: rgb(0x8a8a8a),
    border: rgb(0xdcdcdc),
    accent: rgb(0x2f6fdb),
    selection: rgb(0xcfe0fb),
    active_line: rgb(0xf7f7f7),
    danger: rgb(0xc62828),
    keyword: rgb(0x0033b3),
    string: rgb(0x067d17),
    number: rgb(0x1750eb),
    comment: rgb(0x8c8c8c),
    function: rgb(0x00627a),
    kind: rgb(0x871094),
    property: rgb(0x871094),
    tag: rgb(0x0033b3),
    attribute: rgb(0x174ad4),
};

/// Tema delle app scelto in Impostazioni di Windows → Personalizzazione → Colori.
pub fn system_palette() -> Palette {
    let key: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0".encode_utf16().collect();
    let value: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
    let mut light: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: buffer e dimensione corrispondono a un DWORD.
    let status = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut light as *mut u32).cast(), &mut size)
    };
    if status == 0 && light == 0 {
        DARK
    } else {
        LIGHT
    }
}

//! Primitive GDI per le parti disegnate a mano (tab, status bar, barra di ricerca, intestazione dell'explorer).
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

pub unsafe fn fill(dc: HDC, rect: &RECT, color: u32) {
    let brush = CreateSolidBrush(color);
    FillRect(dc, rect, brush);
    DeleteObject(brush);
}

pub unsafe fn frame(dc: HDC, rect: &RECT, color: u32) {
    let brush = CreateSolidBrush(color);
    FrameRect(dc, rect, brush);
    DeleteObject(brush);
}

pub unsafe fn text(dc: HDC, text: &str, rect: &RECT, color: u32, flags: DRAW_TEXT_FORMAT) {
    // DrawTextW con le ellissi legge il buffer anche quando la lunghezza è 0: con un Vec vuoto il puntatore non è valido.
    if text.is_empty() {
        return;
    }
    let mut text: Vec<u16> = text.encode_utf16().collect();
    let mut rect = *rect;
    SetTextColor(dc, color);
    DrawTextW(dc, text.as_mut_ptr(), text.len() as i32, &mut rect, flags | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
}

pub unsafe fn text_width(dc: HDC, text: &str) -> i32 {
    let text: Vec<u16> = text.encode_utf16().collect();
    let mut size = std::mem::zeroed();
    GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut size);
    size.cx
}

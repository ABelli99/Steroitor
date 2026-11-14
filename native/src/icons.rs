//! Icone dell'explorer come nella versione Tauri: gli stessi glifi SVG 16×16 (`fileGlyphs.ts`), disegnati con un path
//! GDI a 4× e ridotti con la media (antialiasing), in un'ImageList a 32 bit con trasparenza.
use std::path::Path;

use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::{ImageList_Add, ImageList_Create, HIMAGELIST, ILC_COLOR32};

use crate::theme::Palette;

/// Una parte del glifo: path SVG, piena o a tratto, colore fisso (0xRRGGBB) o quello dell'icona, rotazione in gradi.
struct Part {
    d: &'static str,
    fill: bool,
    color: Option<u32>,
    rotate: f64,
}

const fn stroke(d: &'static str) -> Part {
    Part { d, fill: false, color: None, rotate: 0.0 }
}

const fn filled(d: &'static str) -> Part {
    Part { d, fill: true, color: None, rotate: 0.0 }
}

const FRAME: &str = "M2.5 2.5h11a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1z";
const ELLIPSE: &str = "M2 8a6 2.4 0 1 0 12 0a6 2.4 0 1 0-12 0";
const PYTHON_HALF: &str = "M8 1.5c-2.5 0-3 1-3 2v1.5h3.2v.6H3.6c-1.3 0-2.1 1-2.1 2.9s.8 2.9 2.1 2.9H5V9.6c0-1.1.9-2 2-2h3c.9 0 1.5-.7 1.5-1.5V3.5c0-1-1-2-3.5-2z";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Glyph {
    Code,
    Braces,
    Atom,
    Python,
    Cup,
    Kotlin,
    Gear,
    Hexagon,
    Shield,
    Brackets,
    Bezier,
    Hash,
    List,
    Sliders,
    Markdown,
    Database,
    Terminal,
    Image,
    Lock,
    Git,
    Docker,
    Vue,
    Svelte,
    Gem,
    Text,
    File,
}

fn parts(glyph: Glyph) -> Vec<Part> {
    match glyph {
        Glyph::Code => vec![stroke("M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5M9 3l-2 10")],
        Glyph::Braces => vec![stroke("M6 2.5H5A1.5 1.5 0 0 0 3.5 4v2.5L2 8l1.5 1.5V12A1.5 1.5 0 0 0 5 13.5h1M10 2.5h1A1.5 1.5 0 0 1 12.5 4v2.5L14 8l-1.5 1.5V12a1.5 1.5 0 0 1-1.5 1.5h-1")],
        Glyph::Atom => vec![
            stroke(ELLIPSE),
            Part { rotate: 60.0, ..stroke(ELLIPSE) },
            Part { rotate: 120.0, ..stroke(ELLIPSE) },
            filled("M8 7a1 1 0 1 0 0 2a1 1 0 1 0 0-2z"),
        ],
        Glyph::Python => vec![Part { color: Some(0x3572a5), ..filled(PYTHON_HALF) }, Part { color: Some(0xe8c547), rotate: 180.0, ..filled(PYTHON_HALF) }],
        Glyph::Cup => vec![stroke("M3 7h8v3.5a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3zM11 8h1a1.5 1.5 0 0 1 0 3h-1M6 2c-.8 1 .8 2 0 3M8.5 2c-.8 1 .8 2 0 3")],
        Glyph::Kotlin => vec![filled("M2.5 2.5h11L8 8l5.5 5.5h-11z")],
        Glyph::Gear => vec![stroke("M8 4a4 4 0 1 0 0 8a4 4 0 1 0 0-8zM8 6.5a1.5 1.5 0 1 0 0 3a1.5 1.5 0 1 0 0-3zM12 8h2M2 8h2M8 2v2M8 12v2M10.8 5.2l1.4-1.4M3.8 3.8l1.4 1.4M3.8 12.2l1.4-1.4M10.8 10.8l1.4 1.4")],
        Glyph::Hexagon => vec![stroke("M8 1.5l5.6 3.25v6.5L8 14.5l-5.6-3.25v-6.5zM10 6.3a2.5 2.5 0 1 0 0 3.4")],
        Glyph::Shield => vec![stroke("M2.5 1.5h11l-1 11L8 14.5l-4.5-2zM10.5 4.5h-5l.3 3h4l-.3 3-1.5.6-1.5-.6-.1-1")],
        Glyph::Brackets => vec![stroke("M5.5 4L1.5 8l4 4M10.5 4l4 4-4 4")],
        Glyph::Bezier => vec![stroke("M3 12.5C5 3 11 13 13 3.5"), filled("M1.5 11h3v3h-3zM11.5 2h3v3h-3z")],
        Glyph::Hash => vec![stroke("M6.5 2.5l-1.5 11M11.5 2.5l-1.5 11M2.5 6h11M2 10.5h11")],
        Glyph::List => vec![stroke("M2.5 3.5h5M4.5 6.5h9M4.5 9.5h7M2.5 12.5h5")],
        Glyph::Sliders => vec![
            stroke("M2.5 4.5h11M2.5 8h11M2.5 11.5h11"),
            filled("M5 2.9a1.6 1.6 0 1 0 0 3.2a1.6 1.6 0 1 0 0-3.2zM10 6.4a1.6 1.6 0 1 0 0 3.2a1.6 1.6 0 1 0 0-3.2zM7 9.9a1.6 1.6 0 1 0 0 3.2a1.6 1.6 0 1 0 0-3.2z"),
        ],
        Glyph::Markdown => vec![stroke("M2.5 3.5h11a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1zM3.8 10V6l1.7 2 1.7-2v4M11 6v4M9.5 8.5L11 10l1.5-1.5")],
        Glyph::Database => vec![stroke("M3 4c0-1.1 2.2-2 5-2s5 .9 5 2-2.2 2-5 2-5-.9-5-2zM3 4v8c0 1.1 2.2 2 5 2s5-.9 5-2V4M3 8c0 1.1 2.2 2 5 2s5-.9 5-2")],
        Glyph::Terminal => vec![stroke(FRAME), stroke("M4.5 6l2 2-2 2M8 10.5h3.5")],
        Glyph::Image => vec![stroke(FRAME), stroke("M1.5 11.5l4-4 3 3 2-2 4 4"), filled("M11 4.3a1.2 1.2 0 1 0 0 2.4a1.2 1.2 0 1 0 0-2.4z")],
        Glyph::Lock => vec![stroke("M4.5 7.5h7a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-5a1 1 0 0 1 1-1zM5.5 7.5V5a2.5 2.5 0 0 1 5 0v2.5M8 10v2")],
        Glyph::Git => vec![filled("M8 1.2l6.8 6.8L8 14.8 1.2 8z"), Part { color: Some(0xffffff), ..stroke("M8 4.8v6.4M8 6.6l2.2 2.2") }],
        Glyph::Docker => vec![stroke("M1.5 8.5h12.5c-.5 3-3 5-7 5-3 0-5-2-5.5-5z"), filled("M3.5 5.5h2v2h-2zM6.25 5.5h2v2h-2zM9 5.5h2v2H9zM6.25 2.75h2v2h-2z")],
        Glyph::Vue => vec![filled("M.8 2.5h3.1L8 9.6l4.1-7.1h3.1L8 14.5z"), Part { color: Some(0x35495e), ..filled("M4.4 2.5h2L8 5.3l1.6-2.8h2L8 8.7z") }],
        Glyph::Svelte => vec![stroke("M11 3.5c-1.5-1.5-4-1.5-5.5 0S4 7 6 8s4.5 1.5 4.5 3.5-2.5 2.5-4.5 1.5")],
        Glyph::Gem => vec![stroke("M4.5 2.5h7l3 3.5L8 14 1.5 6zM1.5 6h13M6 2.5L5 6l3 8 3-8-1-3.5")],
        Glyph::Text => vec![stroke("M3.5 1.5h6l3 3v10h-9zM9.5 1.5v3h3M5.5 7.5h5M5.5 10h5M5.5 12.5h3")],
        Glyph::File => vec![stroke("M3.5 2.5h6l3 3v8h-9zM9.5 2.5v3h3")],
    }
}

/// Glifo e colore (0xRRGGBB; None = colore attenuato del tema) per nome o estensione, come `fileIcon` della versione Tauri.
pub fn file_icon(path: &Path) -> (Glyph, Option<u32>) {
    let name = path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
    match name.as_str() {
        ".gitignore" | ".gitattributes" | ".gitmodules" => return (Glyph::Git, Some(0xf05033)),
        ".env" => return (Glyph::Sliders, None),
        "dockerfile" => return (Glyph::Docker, Some(0x1d63ed)),
        _ => {}
    }
    let Some(dot) = name.rfind('.').filter(|&dot| dot > 0) else { return (Glyph::File, None) };
    let color = |glyph: Glyph, color: u32| (glyph, Some(color));
    match &name[dot + 1..] {
        "js" | "mjs" | "cjs" => color(Glyph::Code, 0xd4a72c),
        "ts" | "mts" | "cts" => color(Glyph::Code, 0x3178c6),
        "jsx" | "tsx" => color(Glyph::Atom, 0x1ba2c7),
        "py" | "pyw" => (Glyph::Python, Some(0)),
        "java" => color(Glyph::Cup, 0xe76f00),
        "kt" | "kts" => color(Glyph::Kotlin, 0x7f52ff),
        "rs" => color(Glyph::Gear, 0xb7410e),
        "go" => color(Glyph::Code, 0x00add8),
        "c" | "h" => color(Glyph::Hexagon, 0x6b7fa8),
        "cpp" | "cc" | "hpp" => color(Glyph::Hexagon, 0x00599c),
        "cs" => color(Glyph::Hexagon, 0x9b4f96),
        "rb" => color(Glyph::Gem, 0xcc342d),
        "php" => color(Glyph::Code, 0x777bb3),
        "swift" => color(Glyph::Code, 0xf05138),
        "svelte" => color(Glyph::Svelte, 0xff3e00),
        "vue" => color(Glyph::Vue, 0x41b883),
        "html" | "htm" => color(Glyph::Shield, 0xe0632e),
        "xml" => color(Glyph::Brackets, 0xe0632e),
        "svg" => color(Glyph::Bezier, 0xd4a017),
        "css" => color(Glyph::Hash, 0x2965f1),
        "scss" => color(Glyph::Hash, 0xcd6799),
        "less" => color(Glyph::Hash, 0x3d6fb0),
        "json" | "jsonc" => color(Glyph::Braces, 0xc99a2e),
        "yml" | "yaml" => color(Glyph::List, 0xcb4b3e),
        "toml" => color(Glyph::List, 0x9c4221),
        "ini" | "env" | "conf" => (Glyph::Sliders, None),
        "md" | "markdown" => color(Glyph::Markdown, 0x4a7fc1),
        "sql" => color(Glyph::Database, 0x336791),
        "sh" | "bash" | "zsh" => color(Glyph::Terminal, 0x4e9a3f),
        "ps1" => color(Glyph::Terminal, 0x2671be),
        "bat" | "cmd" => (Glyph::Terminal, None),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" => color(Glyph::Image, 0x9b59b6),
        "lock" => (Glyph::Lock, None),
        "txt" | "log" => (Glyph::Text, None),
        _ => (Glyph::File, None),
    }
}

#[derive(Clone, Copy, Debug)]
enum Segment {
    Move(f64, f64),
    Line(f64, f64),
    Cubic([f64; 6]),
    Close,
}

fn numbers(text: &str) -> Vec<f64> {
    let bytes = text.as_bytes();
    let mut values = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'-' || byte == b'.' || byte.is_ascii_digit() {
            let start = index;
            index += 1;
            let mut dot = byte == b'.';
            while index < bytes.len() {
                let next = bytes[index];
                if next.is_ascii_digit() {
                    index += 1;
                } else if next == b'.' && !dot {
                    dot = true;
                    index += 1;
                } else if (next == b'e' || next == b'E') && index + 1 < bytes.len() {
                    index += 2;
                } else {
                    break;
                }
            }
            values.push(text[start..index].parse().unwrap_or(0.0));
        } else {
            index += 1;
        }
    }
    values
}

/// Arco SVG (parametri dell'estremo) in curve cubiche, al più 90° ciascuna.
#[allow(clippy::too_many_arguments)]
fn arc(from: (f64, f64), rx: f64, ry: f64, rotation: f64, large: bool, sweep: bool, to: (f64, f64), out: &mut Vec<Segment>) {
    if (rx == 0.0 || ry == 0.0) || from == to {
        out.push(Segment::Line(to.0, to.1));
        return;
    }
    let (sin, cos) = rotation.to_radians().sin_cos();
    let (dx, dy) = ((from.0 - to.0) / 2.0, (from.1 - to.1) / 2.0);
    let (x1, y1) = (cos * dx + sin * dy, -sin * dx + cos * dy);
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let numerator = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let mut factor = (numerator / (rx * rx * y1 * y1 + ry * ry * x1 * x1)).sqrt();
    if large == sweep {
        factor = -factor;
    }
    let (cx1, cy1) = (factor * rx * y1 / ry, -factor * ry * x1 / rx);
    let center = (cos * cx1 - sin * cy1 + (from.0 + to.0) / 2.0, sin * cx1 + cos * cy1 + (from.1 + to.1) / 2.0);
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let sign = if ux * vy - uy * vx < 0.0 { -1.0 } else { 1.0 };
        let dot = (ux * vx + uy * vy) / ((ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt());
        sign * dot.clamp(-1.0, 1.0).acos()
    };
    let start = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut delta = angle((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !sweep && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f64::consts::TAU;
    }
    let pieces = (delta.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = delta / pieces as f64;
    let point = |theta: f64| {
        let (sin_t, cos_t) = theta.sin_cos();
        (center.0 + rx * cos * cos_t - ry * sin * sin_t, center.1 + rx * sin * cos_t + ry * cos * sin_t)
    };
    let derivative = |theta: f64| {
        let (sin_t, cos_t) = theta.sin_cos();
        (-rx * cos * sin_t - ry * sin * cos_t, -rx * sin * sin_t + ry * cos * cos_t)
    };
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    for piece in 0..pieces {
        let (a, b) = (start + step * piece as f64, start + step * (piece + 1) as f64);
        let (p0, p3, d0, d3) = (point(a), point(b), derivative(a), derivative(b));
        out.push(Segment::Cubic([p0.0 + k * d0.0, p0.1 + k * d0.1, p3.0 - k * d3.0, p3.1 - k * d3.1, p3.0, p3.1]));
    }
}

/// Path SVG (comandi M L H V C S A Z, assoluti e relativi) in segmenti assoluti.
fn parse_path(d: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let (mut x, mut y, mut start) = (0.0, 0.0, (0.0, 0.0));
    let mut last_control: Option<(f64, f64)> = None;
    let mut chunks: Vec<(char, &str)> = Vec::new();
    let mut begin = None;
    for (index, ch) in d.char_indices() {
        if ch.is_ascii_alphabetic() && ch != 'e' && ch != 'E' {
            if let Some((command, at)) = begin {
                chunks.push((command, &d[at..index]));
            }
            begin = Some((ch, index + 1));
        }
    }
    if let Some((command, at)) = begin {
        chunks.push((command, &d[at..]));
    }
    for (command, args) in chunks {
        let values = numbers(args);
        let relative = command.is_ascii_lowercase();
        match command.to_ascii_uppercase() {
            'M' => {
                for (index, pair) in values.chunks(2).filter(|pair| pair.len() == 2).enumerate() {
                    let (px, py) = if relative { (x + pair[0], y + pair[1]) } else { (pair[0], pair[1]) };
                    (x, y) = (px, py);
                    if index == 0 {
                        start = (x, y);
                        out.push(Segment::Move(x, y));
                    } else {
                        out.push(Segment::Line(x, y));
                    }
                }
                last_control = None;
            }
            'L' => {
                for pair in values.chunks(2).filter(|pair| pair.len() == 2) {
                    (x, y) = if relative { (x + pair[0], y + pair[1]) } else { (pair[0], pair[1]) };
                    out.push(Segment::Line(x, y));
                }
                last_control = None;
            }
            'H' => {
                for value in &values {
                    x = if relative { x + value } else { *value };
                    out.push(Segment::Line(x, y));
                }
                last_control = None;
            }
            'V' => {
                for value in &values {
                    y = if relative { y + value } else { *value };
                    out.push(Segment::Line(x, y));
                }
                last_control = None;
            }
            'C' => {
                for c in values.chunks(6).filter(|c| c.len() == 6) {
                    let (bx, by) = if relative { (x, y) } else { (0.0, 0.0) };
                    let points = [bx + c[0], by + c[1], bx + c[2], by + c[3], bx + c[4], by + c[5]];
                    out.push(Segment::Cubic(points));
                    last_control = Some((points[2], points[3]));
                    (x, y) = (points[4], points[5]);
                }
            }
            'S' => {
                for c in values.chunks(4).filter(|c| c.len() == 4) {
                    let (bx, by) = if relative { (x, y) } else { (0.0, 0.0) };
                    let first = last_control.map_or((x, y), |(cx, cy)| (2.0 * x - cx, 2.0 * y - cy));
                    let points = [first.0, first.1, bx + c[0], by + c[1], bx + c[2], by + c[3]];
                    out.push(Segment::Cubic(points));
                    last_control = Some((points[2], points[3]));
                    (x, y) = (points[4], points[5]);
                }
            }
            'A' => {
                for a in values.chunks(7).filter(|a| a.len() == 7) {
                    let to = if relative { (x + a[5], y + a[6]) } else { (a[5], a[6]) };
                    arc((x, y), a[0], a[1], a[2], a[3] != 0.0, a[4] != 0.0, to, &mut out);
                    (x, y) = to;
                }
                last_control = None;
            }
            'Z' => {
                out.push(Segment::Close);
                (x, y) = start;
                last_control = None;
            }
            _ => {}
        }
    }
    out
}

/// Copertura (0..=255) di una parte, a `size` pixel per lato: path GDI a 4× su bitmap bianco/nero, poi media 4×4.
fn coverage(part: &Part, size: i32) -> Vec<u8> {
    const SUPER: i32 = 4;
    let big = size * SUPER;
    let scale = big as f64 / 16.0;
    let (sin, cos) = part.rotate.to_radians().sin_cos();
    let map = |x: f64, y: f64| {
        let (dx, dy) = (x - 8.0, y - 8.0);
        let (rx, ry) = (8.0 + dx * cos - dy * sin, 8.0 + dx * sin + dy * cos);
        POINT { x: (rx * scale).round() as i32, y: (ry * scale).round() as i32 }
    };
    let mut bits = vec![0u8; (big * big * 4) as usize];
    // SAFETY: DC e bitmap in memoria creati e distrutti qui; il buffer ha le dimensioni del bitmap.
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = big;
        info.bmiHeader.biHeight = -big;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut pixels: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, std::ptr::null_mut(), 0);
        let previous = SelectObject(dc, bitmap);
        let black = CreateSolidBrush(0);
        FillRect(dc, &RECT { left: 0, top: 0, right: big, bottom: big }, black);
        DeleteObject(black);
        let white = CreateSolidBrush(0xFFFFFF);
        let width = ((1.2 * scale).round() as u32).max(1);
        let brush = LOGBRUSH { lbStyle: BS_SOLID, lbColor: 0xFFFFFF, lbHatch: 0 };
        let pen = ExtCreatePen((PS_GEOMETRIC | PS_ENDCAP_ROUND | PS_JOIN_ROUND) as u32, width, &brush, 0, std::ptr::null());
        let (old_pen, old_brush) = (SelectObject(dc, pen), SelectObject(dc, white));
        SetPolyFillMode(dc, WINDING as i32);
        BeginPath(dc);
        for segment in parse_path(part.d) {
            match segment {
                Segment::Move(x, y) => {
                    let p = map(x, y);
                    MoveToEx(dc, p.x, p.y, std::ptr::null_mut());
                }
                Segment::Line(x, y) => {
                    let p = map(x, y);
                    LineTo(dc, p.x, p.y);
                }
                Segment::Cubic(c) => {
                    let points = [map(c[0], c[1]), map(c[2], c[3]), map(c[4], c[5])];
                    PolyBezierTo(dc, points.as_ptr(), 3);
                }
                Segment::Close => {
                    CloseFigure(dc);
                }
            }
        }
        EndPath(dc);
        if part.fill {
            FillPath(dc);
        } else {
            StrokePath(dc);
        }
        GdiFlush();
        std::ptr::copy_nonoverlapping(pixels as *const u8, bits.as_mut_ptr(), bits.len());
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(pen);
        DeleteObject(white);
        SelectObject(dc, previous);
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
    }
    let mut result = vec![0u8; (size * size) as usize];
    for y in 0..size {
        for x in 0..size {
            let mut sum = 0u32;
            for sy in 0..SUPER {
                for sx in 0..SUPER {
                    let index = (((y * SUPER + sy) * big + x * SUPER + sx) * 4) as usize;
                    sum += u32::from(bits[index]);
                }
            }
            result[(y * size + x) as usize] = (sum / (SUPER * SUPER) as u32) as u8;
        }
    }
    result
}

/// Immagine BGRA premoltiplicata delle parti, una sopra l'altra, ognuna nel suo colore (0xRRGGBB).
fn render(parts: &[(Part, u32)], size: i32) -> Vec<u32> {
    let mut pixels = vec![0u32; (size * size) as usize];
    for (part, color) in parts {
        let source = [(color >> 16) & 0xFF, (color >> 8) & 0xFF, color & 0xFF];
        for (pixel, alpha) in pixels.iter_mut().zip(coverage(part, size)) {
            let a = u32::from(alpha);
            if a == 0 {
                continue;
            }
            let under = *pixel;
            let over = |value: u32, shift: u32| value * a / 255 + ((under >> shift) & 0xFF) * (255 - a) / 255;
            *pixel = (over(255, 24) << 24) | (over(source[0], 16) << 16) | (over(source[1], 8) << 8) | over(source[2], 0);
        }
    }
    pixels
}

fn rgb_of(colorref: u32) -> u32 {
    ((colorref & 0xFF) << 16) | (colorref & 0xFF00) | ((colorref >> 16) & 0xFF)
}

/// Indici nell'ImageList dell'icona normale e di quella da aperta (diverse solo per le cartelle).
pub fn image_of(path: &Path, is_dir: bool) -> (i32, i32) {
    if is_dir {
        return (FOLDER, FOLDER_OPEN);
    }
    let icon = file_icon(path);
    let variants = || VARIANTS.iter().chain(EXTRA.iter());
    let index = variants().position(|variant| *variant == icon).or_else(|| variants().position(|variant| variant.0 == Glyph::File)).unwrap_or(0) as i32 + 2;
    (index, index)
}

pub const FOLDER: i32 = 0;
pub const FOLDER_OPEN: i32 = 1;

const VARIANTS: [(Glyph, Option<u32>); 34] = [
    (Glyph::Code, Some(0xd4a72c)), (Glyph::Code, Some(0x3178c6)), (Glyph::Atom, Some(0x1ba2c7)), (Glyph::Python, Some(0)),
    (Glyph::Cup, Some(0xe76f00)), (Glyph::Kotlin, Some(0x7f52ff)), (Glyph::Gear, Some(0xb7410e)), (Glyph::Code, Some(0x00add8)),
    (Glyph::Hexagon, Some(0x6b7fa8)), (Glyph::Hexagon, Some(0x00599c)), (Glyph::Hexagon, Some(0x9b4f96)), (Glyph::Gem, Some(0xcc342d)),
    (Glyph::Code, Some(0x777bb3)), (Glyph::Code, Some(0xf05138)), (Glyph::Svelte, Some(0xff3e00)), (Glyph::Vue, Some(0x41b883)),
    (Glyph::Shield, Some(0xe0632e)), (Glyph::Brackets, Some(0xe0632e)), (Glyph::Bezier, Some(0xd4a017)), (Glyph::Hash, Some(0x2965f1)),
    (Glyph::Hash, Some(0xcd6799)), (Glyph::Hash, Some(0x3d6fb0)), (Glyph::Braces, Some(0xc99a2e)), (Glyph::List, Some(0xcb4b3e)),
    (Glyph::List, Some(0x9c4221)), (Glyph::Sliders, None), (Glyph::Markdown, Some(0x4a7fc1)), (Glyph::Database, Some(0x336791)),
    (Glyph::Terminal, Some(0x4e9a3f)), (Glyph::Terminal, Some(0x2671be)), (Glyph::Terminal, None), (Glyph::Image, Some(0x9b59b6)),
    (Glyph::Lock, None), (Glyph::Text, None),
];
const EXTRA: [(Glyph, Option<u32>); 3] = [(Glyph::Git, Some(0xf05033)), (Glyph::Docker, Some(0x1d63ed)), (Glyph::File, None)];

/// ImageList dell'explorer: cartella, cartella aperta e un'icona per ogni coppia glifo/colore, a `size` pixel.
pub fn image_list(palette: &Palette, size: i32) -> HIMAGELIST {
    {
        let (fill, stroke_color) = if palette.dark { (0x43454a, 0x9da0a8) } else { (0xdfe1e5, 0x6c707e) };
        let muted = rgb_of(palette.muted);
        let folder = |d: &'static str| [(Part { d, fill: true, color: None, rotate: 0.0 }, fill), (Part { d, fill: false, color: None, rotate: 0.0 }, stroke_color)];
        let closed = folder("M1.5 4.5a1 1 0 0 1 1-1h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z");
        let mut open: Vec<(Part, u32)> = folder("M1.5 12.5v-8a1 1 0 0 1 1-1h3.6l1.5 1.5h5a1 1 0 0 1 1 1v1.5").into();
        open.extend(folder("M1.5 12.5l2-5h11l-2 5z"));
        let variants: Vec<(Glyph, Option<u32>)> = VARIANTS.iter().chain(EXTRA.iter()).copied().collect();
        // SAFETY: ImageList e bitmap creati qui; l'ImageList copia il bitmap, che poi si elimina.
        let list = unsafe { ImageList_Create(size, size, ILC_COLOR32, (variants.len() + 2) as i32, 4) };
        let add = |pixels: Vec<u32>| {
            // SAFETY: buffer delle dimensioni dichiarate; il bitmap si elimina dopo la copia nell'ImageList.
            unsafe {
                let bitmap = CreateBitmap(size, size, 1, 32, pixels.as_ptr().cast());
                ImageList_Add(list, bitmap, std::ptr::null_mut());
                DeleteObject(bitmap);
            }
        };
        add(render(&closed, size));
        add(render(&open, size));
        for &(glyph, color) in &variants {
            let tint = match color {
                None => muted,
                Some(color) => color,
            };
            let parts: Vec<(Part, u32)> = parts(glyph).into_iter().map(|part| {
                let color = part.color.unwrap_or(tint);
                (part, color)
            }).collect();
            add(render(&parts, size));
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_files_like_the_tauri_version() {
        assert_eq!(file_icon(Path::new("a/App.TSX")), (Glyph::Atom, Some(0x1ba2c7)));
        assert_eq!(file_icon(Path::new(".gitignore")), (Glyph::Git, Some(0xf05033)));
        assert_eq!(file_icon(Path::new("Dockerfile")), (Glyph::Docker, Some(0x1d63ed)));
        assert_eq!(file_icon(Path::new(".bashrc")), (Glyph::File, None));
        assert_eq!(file_icon(Path::new("notes.txt")), (Glyph::Text, None));
        assert_eq!(file_icon(Path::new("x.unknown")), (Glyph::File, None));
    }

    #[test]
    fn every_mapped_icon_has_an_image() {
        let variants: Vec<(Glyph, Option<u32>)> = VARIANTS.iter().chain(EXTRA.iter()).copied().collect();
        for name in ["a.js", "a.ts", "a.jsx", "a.py", "a.java", "a.kt", "a.rs", "a.go", "a.c", "a.cpp", "a.cs", "a.rb", "a.php", "a.swift", "a.svelte", "a.vue", "a.html", "a.xml", "a.svg", "a.css", "a.scss", "a.less", "a.json", "a.yml", "a.toml", "a.ini", "a.md", "a.sql", "a.sh", "a.ps1", "a.bat", "a.png", "a.lock", "a.txt", ".gitignore", "dockerfile", "x"] {
            assert!(variants.contains(&file_icon(Path::new(name))), "{name}");
        }
    }

    #[test]
    fn parses_relative_paths_and_arcs() {
        let segments = parse_path("M2.5 2.5h11a1 1 0 0 1 1 1v9z");
        assert!(matches!(segments[0], Segment::Move(x, y) if x == 2.5 && y == 2.5));
        assert!(matches!(segments[1], Segment::Line(x, y) if x == 13.5 && y == 2.5));
        assert!(matches!(segments.last(), Some(Segment::Close)));
        let Segment::Cubic(end) = segments[2] else { panic!("arco") };
        assert!((end[4] - 14.5).abs() < 1e-9 && (end[5] - 3.5).abs() < 1e-9);
        assert_eq!(numbers("1.6-1.6.8.8"), vec![1.6, -1.6, 0.8, 0.8]);
    }

    /// Tavola di tutte le icone su sfondo scuro e chiaro, in un PNG da guardare: \`STEROITOR_ICONS=percorso.png cargo test -- --ignored icon_sheet\`.
    #[test]
    #[ignore]
    fn icon_sheet() {
        let Some(target) = std::env::var_os("STEROITOR_ICONS") else { return };
        let size = 32;
        let mut icons: Vec<Vec<(Part, u32)>> = Vec::new();
        for &(glyph, color) in VARIANTS.iter().chain(EXTRA.iter()) {
            icons.push(parts(glyph).into_iter().map(|part| {
                let tint = color.unwrap_or(0x8a8a8a);
                let color = part.color.unwrap_or(tint);
                (part, color)
            }).collect());
        }
        let folder = |d: &'static str| vec![(Part { d, fill: true, color: None, rotate: 0.0 }, 0x43454a), (Part { d, fill: false, color: None, rotate: 0.0 }, 0x9da0a8)];
        icons.insert(0, folder("M1.5 4.5a1 1 0 0 1 1-1h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"));
        let (columns, cell) = (10usize, size as usize + 8);
        let rows = icons.len().div_ceil(columns) * 2;
        let (width, height) = (columns * cell, rows * cell);
        let mut image = vec![0u8; width * height * 3];
        for (band, background) in [(0usize, 0x1e1f22u32), (1, 0xffffff)] {
            for (index, icon) in icons.iter().enumerate() {
                let (cx, cy) = ((index % columns) * cell + 4, (band * icons.len().div_ceil(columns) + index / columns) * cell + 4);
                let pixels = render(icon, size);
                for y in 0..cell {
                    for x in 0..cell {
                        let (px, py) = ((index % columns) * cell + x, (band * icons.len().div_ceil(columns) + index / columns) * cell + y);
                        let offset = (py * width + px) * 3;
                        let inside = x >= 4 && y >= 4 && x < 4 + size as usize && y < 4 + size as usize;
                        let pixel = if inside { pixels[(y - 4) * size as usize + x - 4] } else { 0 };
                        let a = (pixel >> 24) & 0xFF;
                        for (channel, shift) in [(0usize, 16u32), (1, 8), (2, 0)] {
                            let back = (background >> shift) & 0xFF;
                            image[offset + channel] = (((pixel >> shift) & 0xFF) + back * (255 - a) / 255).min(255) as u8;
                        }
                        let _ = (cx, cy);
                    }
                }
            }
        }
        // PNG non compresso: blocchi deflate "stored", con CRC32 e Adler-32.
        let crc = |data: &[u8]| {
            let mut crc = 0xFFFF_FFFFu32;
            for &byte in data {
                crc ^= u32::from(byte);
                for _ in 0..8 {
                    crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
                }
            }
            !crc
        };
        let mut raw = Vec::new();
        for row in image.chunks(width * 3) {
            raw.push(0);
            raw.extend_from_slice(row);
        }
        let mut zlib = vec![0x78, 0x01];
        for (index, block) in raw.chunks(65_535).enumerate() {
            let last = (index + 1) * 65_535 >= raw.len();
            zlib.push(u8::from(last));
            zlib.extend_from_slice(&(block.len() as u16).to_le_bytes());
            zlib.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
            zlib.extend_from_slice(block);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in &raw {
            a = (a + u32::from(byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut chunk = |kind: &[u8], data: &[u8]| {
            png.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            png.extend_from_slice(&body);
            png.extend_from_slice(&crc(&body).to_be_bytes());
        };
        let mut header = Vec::new();
        header.extend_from_slice(&(width as u32).to_be_bytes());
        header.extend_from_slice(&(height as u32).to_be_bytes());
        header.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(b"IHDR", &header);
        chunk(b"IDAT", &zlib);
        chunk(b"IEND", &[]);
        std::fs::write(target, png).unwrap();
    }
}

use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFile {
    content: String,
    encoding: String,
    bom: bool,
}

pub fn decode(bytes: &[u8]) -> TextFile {
    if let Some((encoding, bom_len)) = Encoding::for_bom(bytes) {
        let (text, _) = encoding.decode_without_bom_handling(&bytes[bom_len..]);
        return TextFile { content: text.into_owned(), encoding: encoding.name().into(), bom: true };
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return TextFile { content: text.to_owned(), encoding: UTF_8.name().into(), bom: false };
    }
    let (text, _, _) = WINDOWS_1252.decode(bytes);
    TextFile { content: text.into_owned(), encoding: WINDOWS_1252.name().into(), bom: false }
}

fn encode_utf16(content: &str, bom: bool, to_bytes: fn(u16) -> [u8; 2]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() * 2 + 2);
    if bom {
        out.extend(to_bytes(0xFEFF));
    }
    content.encode_utf16().for_each(|unit| out.extend(to_bytes(unit)));
    out
}

fn encode(content: &str, label: &str, bom: bool) -> Result<Vec<u8>, String> {
    let encoding = Encoding::for_label(label.as_bytes()).ok_or(format!("Encoding sconosciuto: {label}"))?;
    if encoding == UTF_16LE {
        return Ok(encode_utf16(content, bom, u16::to_le_bytes));
    }
    if encoding == UTF_16BE {
        return Ok(encode_utf16(content, bom, u16::to_be_bytes));
    }

    let (bytes, _, had_errors) = encoding.encode(content);
    if had_errors {
        return Err(format!("Alcuni caratteri non sono rappresentabili in {}", encoding.name()));
    }
    let mut out = Vec::with_capacity(bytes.len() + 3);
    if bom && encoding == UTF_8 {
        out.extend([0xEF, 0xBB, 0xBF]);
    }
    out.extend_from_slice(&bytes);
    Ok(out)
}

#[tauri::command]
pub async fn read_text_file(path: String) -> Result<TextFile, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("Impossibile leggere {path}: {e}"))?;
    Ok(decode(&bytes))
}

#[tauri::command]
pub async fn write_text_file(path: String, content: String, encoding: String, bom: bool) -> Result<(), String> {
    let bytes = encode(&content, &encoding, bom)?;
    std::fs::write(&path, bytes).map_err(|e| format!("Impossibile salvare {path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_every_supported_encoding() {
        let text = "àèìòù — ok\r\nriga 2";
        for (label, bom) in [("UTF-8", false), ("UTF-8", true), ("UTF-16LE", true), ("UTF-16BE", true)] {
            let decoded = decode(&encode(text, label, bom).unwrap());
            assert_eq!(decoded.content, text);
            assert_eq!(decoded.encoding, label);
            assert_eq!(decoded.bom, bom);
        }
    }

    #[test]
    fn falls_back_to_windows_1252_on_invalid_utf8() {
        let decoded = decode(&[0x63, 0x61, 0x66, 0xE8]);
        assert_eq!(decoded.content, "cafè");
        assert_eq!(decoded.encoding, "windows-1252");
    }

    #[test]
    fn rejects_unrepresentable_characters() {
        assert!(encode("日本", "windows-1252", false).is_err());
    }
}

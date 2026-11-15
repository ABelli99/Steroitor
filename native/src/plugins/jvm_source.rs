//! Sorgenti Java e Kotlin letti senza parser: commenti e contenuto delle stringhe diventano spazi, con gli stessi
//! offset del testo, così le ricerche non trovano parole nei commenti o nelle stringhe.

pub fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn find(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    bytes.get(from..)?.windows(needle.len()).position(|window| window == needle).map(|at| from + at)
}

/// Il testo con commenti e contenuto di stringhe e caratteri a spazi; a capo e virgolette restano al loro posto.
pub fn mask(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut blank = |range: std::ops::Range<usize>| {
        for byte in &mut out[range] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
    };
    let mut index = 0;
    while index < bytes.len() {
        let rest = &bytes[index..];
        if rest.starts_with(b"//") {
            let end = find(bytes, index, b"\n").unwrap_or(bytes.len());
            blank(index..end);
            index = end;
        } else if rest.starts_with(b"/*") {
            let end = find(bytes, index + 2, b"*/").map_or(bytes.len(), |at| at + 2);
            blank(index..end);
            index = end;
        } else if rest.starts_with(b"\"\"\"") {
            let close = find(bytes, index + 3, b"\"\"\"");
            blank(index + 3..close.unwrap_or(bytes.len()));
            index = close.map_or(bytes.len(), |at| at + 3);
        } else if rest[0] == b'"' || rest[0] == b'\'' {
            let mut end = index + 1;
            while end < bytes.len() && bytes[end] != rest[0] && bytes[end] != b'\n' {
                end += if bytes[end] == b'\\' { 2 } else { 1 };
            }
            let end = end.min(bytes.len());
            blank(index + 1..end);
            index = end + 1;
        } else {
            index += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blanks_comments_and_strings() {
        let text = "a(\"x // y\", 'z'); // c\n/* d */ b \"\"\"\nq\"\"\" è";
        let masked = String::from_utf8(mask(text)).unwrap();
        assert_eq!(masked.len(), text.len());
        assert_eq!(masked, "a(\"      \", ' ');     \n        b \"\"\"\n \"\"\" è");
    }
}

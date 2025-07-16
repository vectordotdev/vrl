// https://en.wikipedia.org/wiki/Byte_order_mark#UTF-8

/// Helper trait to strip BOM from UTF-8
pub trait StripBomFromUTF8 {
    #[must_use]
    fn strip_bom(self) -> Self;
}

// \u{feff} and [0xef, 0xbb, 0xbf] are the same
static BOM_MARKER_BYTES: &[u8] = &[0xef, 0xbb, 0xbf];
static BOM_MARKER: char = '\u{feff}';

impl StripBomFromUTF8 for &str {
    fn strip_bom(self) -> Self {
        self.trim_start_matches(BOM_MARKER)
    }
}

impl StripBomFromUTF8 for &[u8] {
    fn strip_bom(self) -> Self {
        self.strip_prefix(BOM_MARKER_BYTES).unwrap_or(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_strip_bom_str_from_bytes() {
        let raw = &[BOM_MARKER_BYTES, &[0x7b, 0x7d]].concat(); // BOM{}
        let raw: &str = std::str::from_utf8(raw).unwrap();
        assert_eq!(raw.len(), 5);
        assert_eq!(raw, format!("{BOM_MARKER}{{}}"));

        let stripped = raw.strip_bom();
        assert_eq!(stripped.len(), 2);
        assert_eq!(stripped, "{}");
    }

    #[test]
    fn test_strip_bom_str_with_utf8_escape() {
        let raw = format!("{BOM_MARKER}{{}}"); // BOM{}
        // Should be the exact same as the test from raw bytes
        let raw: &str = raw.as_str();
        assert_eq!(raw.len(), 5);

        let stripped = raw.strip_bom();
        assert_eq!(stripped.len(), 2);
        assert_eq!(stripped, "{}");
    }

    #[test]
    fn test_strip_bom_u8_slice() {
        let raw = &[BOM_MARKER_BYTES, &[0x7b, 0x7d]].concat(); // BOM{}
        assert_eq!(raw.len(), 5);

        let stripped = raw.strip_bom();
        assert_eq!(stripped.len(), 2);
        assert_eq!(stripped, &[0x7b, 0x7d]);
    }
}

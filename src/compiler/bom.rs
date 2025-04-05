// https://en.wikipedia.org/wiki/Byte_order_mark#UTF-8

/// Helper trait to strip BOM from UTF-8
pub trait StripBomUTF8 {
    fn strip_bom(self: Self) -> Self;
}

// \u{feff} and [0xef, 0xbb, 0xbf] are the same
impl StripBomUTF8 for &str {
    fn strip_bom(self: Self) -> Self {
        self.trim_start_matches("\u{feff}")
    }
}

impl StripBomUTF8 for &[u8] {
    fn strip_bom(self: Self) -> Self {
        self.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(self)
    }
}

#[cfg(test)]
mod test {
    use super::StripBomUTF8;

    #[test]
    fn test_strip_bom_str_from_bytes() {
        let raw = &[0xef, 0xbb, 0xbf, 0x7b, 0x7d]; // BOM{}
        let raw: &str = std::str::from_utf8(raw).unwrap();
        assert_eq!(raw.len(), 5);
        assert_eq!(raw, "\u{feff}{}");

        let stripped = raw.strip_bom();
        assert_eq!(stripped.len(), 2);
        assert_eq!(stripped, "{}");
    }

    #[test]
    fn test_strip_bom_str_with_utf8_escape() {
        let raw: &str = "\u{feff}{}"; // BOM{}
                                      // Should be the exact same as the test from raw bytes
        assert_eq!(raw.len(), 5);

        let stripped = raw.strip_bom();
        assert_eq!(stripped.len(), 2);
        assert_eq!(stripped, "{}");
    }

    #[test]
    fn test_strip_bom_u8_slice() {
        let raw: &[u8] = &[0xef, 0xbb, 0xbf, 0x7b, 0x7d]; // BOM{}
        assert_eq!(raw.len(), 5);

        let stripped = raw.strip_bom();
        assert_eq!(stripped.len(), 2);
        assert_eq!(stripped, &[0x7b, 0x7d]);
    }
}

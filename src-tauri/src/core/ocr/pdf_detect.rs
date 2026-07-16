use std::path::Path;

pub fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

pub fn is_pdf_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[..4] == b"%PDF"
}

pub fn looks_like_pdf(path: Option<&Path>, bytes: &[u8]) -> bool {
    path.map(is_pdf_path).unwrap_or(false) || is_pdf_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn path_extension_pdf_case_insensitive() {
        assert!(is_pdf_path(Path::new("a.PDF")));
        assert!(is_pdf_path(Path::new("x/y/z.pdf")));
        assert!(!is_pdf_path(Path::new("a.png")));
        assert!(!is_pdf_path(Path::new("pdf")));
    }

    #[test]
    fn magic_percent_pdf() {
        assert!(is_pdf_bytes(b"%PDF-1.4\n..."));
        assert!(!is_pdf_bytes(b"\x89PNG\r\n"));
        assert!(!is_pdf_bytes(b""));
        assert!(!is_pdf_bytes(b"%PD"));
    }

    #[test]
    fn looks_like_pdf_or_of_path_and_magic() {
        assert!(looks_like_pdf(Some(Path::new("doc.pdf")), b"not-magic"));
        assert!(looks_like_pdf(Some(Path::new("doc.bin")), b"%PDF-1.7"));
        assert!(!looks_like_pdf(Some(Path::new("a.png")), b"\x89PNG"));
    }
}

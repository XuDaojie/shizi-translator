use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrRunMeta {
    pub engine: String,
    pub model: Option<String>,
    pub source_width: u32,
    pub source_height: u32,
    pub sent_width: u32,
    pub sent_height: u32,
    pub png_bytes: Option<u64>,
    pub latency_ms: u64,
    pub http_status: Option<u16>,
    pub scaled: bool,
}

impl OcrRunMeta {
    pub fn info_summary(&self) -> String {
        format!(
            "engine={} model={:?} src={}x{} sent={}x{} png={:?} latency_ms={} http={:?} scaled={}",
            self.engine,
            self.model,
            self.source_width,
            self.source_height,
            self.sent_width,
            self.sent_height,
            self.png_bytes,
            self.latency_ms,
            self.http_status,
            self.scaled
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_summary_contains_core_fields() {
        let m = OcrRunMeta {
            engine: "openai-vision".into(),
            model: Some("gpt-4o".into()),
            source_width: 100,
            source_height: 50,
            sent_width: 100,
            sent_height: 50,
            png_bytes: Some(1234),
            latency_ms: 42,
            http_status: Some(200),
            scaled: false,
        };
        let s = m.info_summary();
        assert!(s.contains("openai-vision"));
        assert!(s.contains("100x50"));
        assert!(s.contains("latency_ms=42"));
    }

    #[test]
    fn serializes_camel_case() {
        let m = OcrRunMeta {
            engine: "windows-media-ocr".into(),
            model: None,
            source_width: 1,
            source_height: 1,
            sent_width: 1,
            sent_height: 1,
            png_bytes: None,
            latency_ms: 0,
            http_status: None,
            scaled: false,
        };
        let v = serde_json::to_value(&m).unwrap();
        assert!(v.get("sourceWidth").is_some());
        assert!(v.get("latencyMs").is_some());
        assert!(v.get("httpStatus").is_some());
    }
}

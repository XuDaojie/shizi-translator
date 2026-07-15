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

/// 纯识别（不翻译）完整响应：正文 + 元信息 + UI 预览 PNG。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognizeImageResponse {
    pub text: String,
    pub meta: OcrRunMeta,
    /// 供 UI 预览的 PNG base64（无 data: 前缀）；勿写入日志
    pub preview_png_base64: String,
}

/// platform 纯识别成功结果：IPC 用 response；source_image 供 ui 写 last_ocr_image。
#[derive(Debug, Clone)]
pub struct RecognizeImageFullResult {
    pub response: RecognizeImageResponse,
    /// 源图拷贝，勿写入日志
    pub source_image: crate::core::capture::CapturedImage,
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

    #[test]
    fn full_result_holds_response_and_source_image() {
        use crate::core::capture::{CapturedImage, CapturedImageFormat};
        let img = CapturedImage {
            bytes: vec![0; 4],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        };
        let response = RecognizeImageResponse {
            text: "hi".into(),
            meta: OcrRunMeta {
                engine: "mock".into(),
                model: None,
                source_width: 1,
                source_height: 1,
                sent_width: 1,
                sent_height: 1,
                png_bytes: None,
                latency_ms: 0,
                http_status: None,
                scaled: false,
            },
            preview_png_base64: "eA==".into(),
        };
        let full = RecognizeImageFullResult {
            response: response.clone(),
            source_image: img.clone(),
        };
        assert_eq!(full.response.text, "hi");
        assert_eq!(full.source_image.width, 1);
        // 确认 RecognizeImageResponse 仍可独立序列化（IPC 形状不变）
        let v = serde_json::to_value(&full.response).unwrap();
        assert!(v.get("previewPngBase64").is_some());
        assert!(v.get("source_image").is_none());
    }
}

pub mod image_encode;
pub mod meta;
pub mod pdf_detect;
pub mod resolve;
pub mod vision_openai;

// cdylib crate-type 无 Rust 外部消费者，pub use re-export 易被判死代码；保留供短路径访问
#[allow(unused_imports)]
pub use image_encode::{
    encode_captured_image_png_info, encode_png_unscaled, EncodePngInfo,
};
#[cfg(test)]
#[allow(unused_imports)]
pub use image_encode::encode_captured_image_png;
#[allow(unused_imports)]
pub use meta::{OcrRunMeta, RecognizeImageFullResult, RecognizeImageResponse};
#[allow(unused_imports)]
pub use resolve::{resolve_ocr_engine, resolve_ocr_engine_for, ResolvedOcrEngine, VisionOcrConfig};

use crate::core::capture::CapturedImage;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OcrHints {
    pub preferred_languages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
    pub engine: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub words: Vec<OcrWord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub bounding_box: OcrBoundingBox,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OcrBoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OcrError {
    #[error("系统 OCR 能力不可用")]
    EngineUnavailable,
    #[error("缺少 OCR 语言包：{0}")]
    LanguageUnavailable(String),
    #[error("截图区域过大，请缩小区域")]
    ImageTooLarge,
    #[error("OCR 图像转换失败：{0}")]
    ImageConversionFailed(String),
    #[error("未识别到文本")]
    EmptyResult,
    #[allow(dead_code)] // 平台抽象预留：非 Windows 路径错误完整性
    #[error("当前平台暂不支持 OCR")]
    UnsupportedPlatform,
    #[error("没有可用的文字识别服务")]
    NoEngineConfigured,
    #[error("不支持的 OCR 协议：{0}")]
    UnsupportedProtocol(String),
    #[error("OCR 认证失败：{0}")]
    Auth(String),
    #[error("OCR 服务错误：{message}")]
    Api { message: String, retryable: bool },
    #[error("OCR 网络错误：{0}")]
    Http(String),
    #[error("OCR 渠道不存在：{0}")]
    UnknownService(String),
    #[error("无法打开 PDF 文件：{0}")]
    PdfOpenFailed(String),
    #[error("PDF 中没有可识别的页面")]
    PdfEmptyDocument,
    #[error("PDF 页面渲染失败：{0}")]
    PdfRenderFailed(String),
}

#[async_trait::async_trait]
pub trait OcrEngine: Send + Sync {
    async fn recognize(&self, image: CapturedImage, hints: OcrHints)
        -> Result<OcrResult, OcrError>;
}

#[cfg(test)]
mod error_tests {
    use super::OcrError;

    #[test]
    fn new_variants_display_messages() {
        let no = OcrError::NoEngineConfigured;
        assert!(no.to_string().contains("没有可用") || no.to_string().contains("文字识别"));

        let unsup = OcrError::UnsupportedProtocol("claude-vision".into());
        assert!(unsup.to_string().contains("claude-vision"));

        let auth = OcrError::Auth("missing key".into());
        assert!(auth.to_string().contains("missing key") || auth.to_string().contains("认证"));

        let api = OcrError::Api {
            message: "rate limit".into(),
            retryable: true,
        };
        assert!(api.to_string().contains("rate limit"));

        let http = OcrError::Http("timeout".into());
        assert!(http.to_string().contains("timeout") || http.to_string().contains("网络"));
    }

    #[test]
    fn pdf_and_unknown_service_variants_display() {
        let u = OcrError::UnknownService("svc-x".into());
        assert!(u.to_string().contains("svc-x") || u.to_string().contains("渠道"));

        let open = OcrError::PdfOpenFailed("bad".into());
        assert!(open.to_string().contains("PDF") || open.to_string().contains("打开"));

        let empty = OcrError::PdfEmptyDocument;
        assert!(empty.to_string().contains("页") || empty.to_string().contains("PDF"));

        let render = OcrError::PdfRenderFailed("x".into());
        assert!(render.to_string().contains("渲染") || render.to_string().contains("PDF"));
    }
}

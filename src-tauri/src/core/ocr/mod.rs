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
}

#[async_trait::async_trait]
pub trait OcrEngine: Send + Sync {
    async fn recognize(&self, image: CapturedImage, hints: OcrHints)
        -> Result<OcrResult, OcrError>;
}

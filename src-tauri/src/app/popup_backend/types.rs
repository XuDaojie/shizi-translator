//! 弹窗后端 ViewModel / 卡片 / 用户动作类型。
//! 纯数据结构，供 WinUI 与 WebView 宿主共用。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupPositionMode {
    #[default]
    NearCursor,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupUiBackendKind {
    Webview,
    Winui,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupCardStatus {
    Pending,
    Translating,
    Finished,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PopupCardVm {
    pub service_instance_id: String,
    pub service_name: String,
    pub service_type: String,
    pub protocol: String,
    pub model_name: String,
    pub status: PopupCardStatus,
    pub text: String,
    pub error_message: String,
    pub usage_input: Option<u32>,
    pub usage_output: Option<u32>,
    pub detected_source_lang: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PopupViewModel {
    pub session_id: Option<String>,
    pub source_text: String,
    pub source_type: String,
    pub source_lang: String,
    pub target_lang: String,
    pub is_translating: bool,
    pub cards: Vec<PopupCardVm>,
}

#[derive(Debug, Clone)]
pub enum PopupUserAction {
    Close,
    CancelTranslation,
    Retry {
        service_instance_id: Option<String>,
    },
    CopyResult {
        service_instance_id: String,
    },
    OpenSettings,
    SetSessionLanguages {
        source_lang: String,
        target_lang: String,
    },
}

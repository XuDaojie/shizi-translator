pub mod microsoft;

/// WebView 初始化时采集的浏览器环境信息，供 MicrosoftMtProvider 拼装请求头。
#[derive(Debug, Clone, Default)]
pub struct EdgeTranslateEnv {
    pub user_agent: String,
    pub accept_language: String, // 如 "zh-CN,zh;q=0.9,en;q=0.8"
}

/// 编译期默认 UA 兜底（当前 Edge 稳定版 UA 字符串），env 未采集到时使用。
/// Edge 更新后此值可能过时，但接口对 UA 版本不敏感（仅用于派生 sec-mesh-client-* 头）。
pub const DEFAULT_EDGE_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 Edg/150.0.0.0";

pub use microsoft::MicrosoftMtProvider;

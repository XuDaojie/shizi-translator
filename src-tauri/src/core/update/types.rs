use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "beta" => Self::Beta,
            _ => Self::Stable,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    /// GitHub Release 附件；检查更新时优先选轻量 NSIS（非 full）。
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone)]
pub struct SelectedRelease {
    pub version: semver::Version,
    pub name: Option<String>,
    /// 优先轻量包直链；无合适 asset 时为 release 页 html_url（或兜底列表页）。
    pub download_url: String,
    pub is_prerelease: bool,
}

/// 前端约定 camelCase：用 rename_all = "camelCase" 在结果 struct 上。
/// status 字符串约定：`"up_to_date" | "update_available" | "error"`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckUpdateResult {
    pub status: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_name: Option<String>,
    pub release_url: Option<String>,
    pub is_prerelease: Option<bool>,
    pub message: Option<String>,
}

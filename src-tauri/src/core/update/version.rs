use super::github::RELEASES_PAGE_FALLBACK;
use super::types::{CheckUpdateResult, ReleaseAsset, ReleaseInfo, SelectedRelease, UpdateChannel};

/// `html_url` 须为 https；否则回退到 releases 列表页。
fn resolve_release_url(html_url: &str) -> String {
    if html_url.starts_with("https://") {
        html_url.to_string()
    } else {
        RELEASES_PAGE_FALLBACK.to_string()
    }
}

/// 是否为轻量 NSIS 安装包（推荐更新通道产物）。
/// 约定：`Shizi_*_x64-setup.exe`；完整包为 `*-setup-full.exe`，不得被选中。
pub fn is_slim_nsis_asset_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with("-setup.exe") && !n.contains("full")
}

/// 从 Release assets 中优先取轻量包直链；否则回退到 release 页（https 校验）。
pub fn prefer_slim_download_url(html_url: &str, assets: &[ReleaseAsset]) -> String {
    for asset in assets {
        if !is_slim_nsis_asset_name(&asset.name) {
            continue;
        }
        let url = asset.browser_download_url.trim();
        if url.starts_with("https://") {
            return url.to_string();
        }
    }
    resolve_release_url(html_url)
}

pub fn parse_tag_version(tag: &str) -> Option<semver::Version> {
    let trimmed = tag.trim();
    let t = trimmed.strip_prefix('v').unwrap_or(trimmed);
    semver::Version::parse(t).ok()
}

pub fn select_latest_for_channel(
    releases: &[ReleaseInfo],
    channel: UpdateChannel,
) -> Option<SelectedRelease> {
    releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = parse_tag_version(&r.tag_name)?;
            let has_semver_pre = !version.pre.is_empty();
            let is_prerelease = r.prerelease || has_semver_pre;

            match channel {
                UpdateChannel::Stable => {
                    if r.prerelease || has_semver_pre {
                        return None;
                    }
                }
                UpdateChannel::Beta => {}
            }

            Some(SelectedRelease {
                version,
                name: r.name.clone(),
                download_url: prefer_slim_download_url(&r.html_url, &r.assets),
                is_prerelease,
            })
        })
        .max_by(|a, b| a.version.cmp(&b.version))
}

pub fn is_update_available(current: &str, latest: &str) -> bool {
    match (parse_tag_version(current), parse_tag_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// CI Nightly 版本形如 `0.7.0-nightly.YYYYMMDD.sha`（pre 首段为 `nightly`）。
/// semver 下 `0.7.0-nightly.*` < `0.7.0`，若仍走常规比较会误报「有正式版可更新」。
pub fn is_nightly_build(version: &str) -> bool {
    parse_tag_version(version)
        .map(|v| {
            v.pre
                .as_str()
                .split('.')
                .next()
                .is_some_and(|id| id.eq_ignore_ascii_case("nightly"))
        })
        .unwrap_or(false)
}

pub fn evaluate_check(
    current_version: &str,
    releases: &[ReleaseInfo],
    channel: UpdateChannel,
) -> CheckUpdateResult {
    // Nightly 不走应用内「升级到正式版/Beta」提示；用户应自行到 Nightly Release 取新包。
    if is_nightly_build(current_version) {
        return CheckUpdateResult {
            status: "up_to_date".into(),
            current_version: current_version.to_string(),
            latest_version: None,
            release_name: None,
            release_url: Some(RELEASES_PAGE_FALLBACK.to_string()),
            is_prerelease: Some(true),
            message: Some(
                "当前为 Nightly 构建：应用内检查更新不会提示正式版/Beta。请从 Nightly Release 获取最新每日构建。"
                    .into(),
            ),
        };
    }

    let Some(latest) = select_latest_for_channel(releases, channel) else {
        return CheckUpdateResult {
            status: "up_to_date".into(),
            current_version: current_version.to_string(),
            latest_version: None,
            release_name: None,
            release_url: None,
            is_prerelease: None,
            message: None,
        };
    };

    let latest_str = latest.version.to_string();
    // 始终优先轻量包直链（应用更新不需要完整包内嵌的 WebView2 安装器）。
    let release_url = Some(latest.download_url);
    if is_update_available(current_version, &latest_str) {
        CheckUpdateResult {
            status: "update_available".into(),
            current_version: current_version.to_string(),
            latest_version: Some(latest_str),
            release_name: latest.name,
            release_url,
            is_prerelease: Some(latest.is_prerelease),
            message: None,
        }
    } else {
        CheckUpdateResult {
            status: "up_to_date".into(),
            current_version: current_version.to_string(),
            latest_version: Some(latest_str),
            release_name: latest.name,
            release_url,
            is_prerelease: Some(latest.is_prerelease),
            message: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::update::types::{ReleaseInfo, UpdateChannel};

    fn asset(name: &str, url: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.into(),
            browser_download_url: url.into(),
        }
    }

    fn release(
        tag: &str,
        html_url: &str,
        prerelease: bool,
        draft: bool,
        assets: Vec<ReleaseAsset>,
    ) -> ReleaseInfo {
        ReleaseInfo {
            tag_name: tag.into(),
            name: None,
            html_url: html_url.into(),
            prerelease,
            draft,
            assets,
        }
    }

    #[test]
    fn parse_tag_strips_v_prefix() {
        assert_eq!(parse_tag_version("v1.2.3").unwrap().to_string(), "1.2.3");
        assert_eq!(parse_tag_version("1.2.3").unwrap().to_string(), "1.2.3");
        assert_eq!(
            parse_tag_version("v0.7.0-beta.5").unwrap().to_string(),
            "0.7.0-beta.5"
        );
        assert!(parse_tag_version("not-a-version").is_none());
        assert!(parse_tag_version("").is_none());
    }

    #[test]
    fn slim_nsis_asset_name_excludes_full() {
        assert!(is_slim_nsis_asset_name("Shizi_0.7.1_x64-setup.exe"));
        assert!(!is_slim_nsis_asset_name("Shizi_0.7.1_x64-setup-full.exe"));
        assert!(!is_slim_nsis_asset_name("Shizi_0.7.1_x64-full-setup.exe"));
        assert!(!is_slim_nsis_asset_name("notes.md"));
    }

    #[test]
    fn prefer_slim_download_skips_full_asset() {
        let html = "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.1";
        let assets = vec![
            asset(
                "Shizi_0.7.1_x64-setup-full.exe",
                "https://github.com/XuDaojie/shizi-translator/releases/download/v0.7.1/Shizi_0.7.1_x64-setup-full.exe",
            ),
            asset(
                "Shizi_0.7.1_x64-setup.exe",
                "https://github.com/XuDaojie/shizi-translator/releases/download/v0.7.1/Shizi_0.7.1_x64-setup.exe",
            ),
        ];
        assert_eq!(
            prefer_slim_download_url(html, &assets),
            "https://github.com/XuDaojie/shizi-translator/releases/download/v0.7.1/Shizi_0.7.1_x64-setup.exe"
        );
    }

    #[test]
    fn prefer_slim_download_falls_back_to_release_page() {
        let html = "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.1";
        assert_eq!(prefer_slim_download_url(html, &[]), html);
        assert_eq!(
            prefer_slim_download_url(
                html,
                &[asset(
                    "Shizi_0.7.1_x64-setup-full.exe",
                    "https://example.com/full.exe"
                )]
            ),
            html
        );
    }

    #[test]
    fn stable_channel_skips_prerelease_flag_and_semver_pre() {
        let releases = vec![
            release(
                "v0.8.0-beta.1",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0-beta.1",
                true,
                false,
                vec![],
            ),
            ReleaseInfo {
                tag_name: "v0.7.1".into(),
                name: Some("stable".into()),
                html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.1".into(),
                prerelease: false,
                draft: false,
                assets: vec![],
            },
            release("bad-tag", "https://github.com/example/x", false, false, vec![]),
        ];
        let latest = select_latest_for_channel(&releases, UpdateChannel::Stable).unwrap();
        assert_eq!(latest.version.to_string(), "0.7.1");
        assert!(!latest.is_prerelease);
    }

    #[test]
    fn beta_channel_picks_highest_including_prerelease() {
        let releases = vec![
            release(
                "v0.7.0",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.0",
                false,
                false,
                vec![],
            ),
            release(
                "v0.7.0-beta.9",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.0-beta.9",
                true,
                false,
                vec![],
            ),
            release(
                "v0.8.0-beta.1",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0-beta.1",
                true,
                false,
                vec![],
            ),
        ];
        // semver: 0.8.0-beta.1 > 0.7.0 > 0.7.0-beta.9
        let latest = select_latest_for_channel(&releases, UpdateChannel::Beta).unwrap();
        assert_eq!(latest.version.to_string(), "0.8.0-beta.1");
        assert!(latest.is_prerelease);
    }

    #[test]
    fn evaluate_check_prefers_slim_asset_url() {
        let slim =
            "https://github.com/XuDaojie/shizi-translator/releases/download/v0.8.0/Shizi_0.8.0_x64-setup.exe";
        let full =
            "https://github.com/XuDaojie/shizi-translator/releases/download/v0.8.0/Shizi_0.8.0_x64-setup-full.exe";
        let releases = vec![release(
            "v0.8.0",
            "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0",
            false,
            false,
            vec![
                asset("Shizi_0.8.0_x64-setup-full.exe", full),
                asset("Shizi_0.8.0_x64-setup.exe", slim),
            ],
        )];
        let result = evaluate_check("0.7.0", &releases, UpdateChannel::Stable);
        assert_eq!(result.status, "update_available");
        assert_eq!(result.release_url.as_deref(), Some(slim));
    }

    #[test]
    fn compare_update_available_when_latest_greater() {
        assert!(is_update_available("0.7.0", "0.7.1"));
        assert!(is_update_available("0.7.0", "0.7.0-beta.6") == false); // 0.7.0 > 0.7.0-beta.6
        assert!(!is_update_available("0.7.0", "0.7.0"));
        assert!(is_update_available("0.7.0-beta.5", "0.7.0-beta.6"));
        // 正是 Nightly 误报根因：同 base 的 pre 小于正式版
        assert!(is_update_available("0.7.0-nightly.20260721.abc", "0.7.0"));
    }

    #[test]
    fn is_nightly_build_detects_pre_id() {
        assert!(is_nightly_build("0.7.0-nightly.20260721.813a439"));
        assert!(is_nightly_build("v0.7.1-nightly.1"));
        assert!(!is_nightly_build("0.7.0"));
        assert!(!is_nightly_build("0.7.0-beta.1"));
        assert!(!is_nightly_build("not-a-version"));
    }

    #[test]
    fn evaluate_check_skips_update_prompt_for_nightly_current() {
        let releases = vec![ReleaseInfo {
            tag_name: "v0.7.0".into(),
            name: Some("stable".into()),
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.0".into(),
            prerelease: false,
            draft: false,
            assets: vec![],
        }];
        let result = evaluate_check(
            "0.7.0-nightly.20260721.813a439",
            &releases,
            UpdateChannel::Stable,
        );
        assert_eq!(result.status, "up_to_date");
        assert!(result.latest_version.is_none());
        assert!(result
            .message
            .as_deref()
            .is_some_and(|m| m.contains("Nightly")));
        // 若未拦截，semver 会判定 0.7.0 > nightly 从而误报
        assert!(is_update_available(
            "0.7.0-nightly.20260721.813a439",
            "0.7.0"
        ));
    }

    #[test]
    fn draft_releases_are_ignored() {
        let releases = vec![release(
            "v9.0.0",
            "https://github.com/XuDaojie/shizi-translator/releases/tag/v9.0.0",
            false,
            true,
            vec![],
        )];
        assert!(select_latest_for_channel(&releases, UpdateChannel::Stable).is_none());
    }

    #[test]
    fn resolve_release_url_requires_https() {
        assert_eq!(
            resolve_release_url("https://github.com/XuDaojie/shizi-translator/releases/tag/v1.0.0"),
            "https://github.com/XuDaojie/shizi-translator/releases/tag/v1.0.0"
        );
        assert_eq!(
            resolve_release_url("http://evil.example/x"),
            RELEASES_PAGE_FALLBACK
        );
        assert_eq!(resolve_release_url(""), RELEASES_PAGE_FALLBACK);
        assert_eq!(
            resolve_release_url("ftp://example.com/x"),
            RELEASES_PAGE_FALLBACK
        );
    }

    #[test]
    fn evaluate_check_falls_back_release_url_when_not_https() {
        let releases = vec![ReleaseInfo {
            tag_name: "v9.9.9".into(),
            name: Some("next".into()),
            html_url: "http://not-https.example/release".into(),
            prerelease: false,
            draft: false,
            assets: vec![],
        }];
        let result = evaluate_check("0.1.0", &releases, UpdateChannel::Stable);
        assert_eq!(result.status, "update_available");
        assert_eq!(
            result.release_url.as_deref(),
            Some(RELEASES_PAGE_FALLBACK)
        );
    }
}

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

/// semver pre 首段是否为 `nightly`（大小写不敏感）。
fn version_has_nightly_pre(version: &semver::Version) -> bool {
    version
        .pre
        .as_str()
        .split('.')
        .next()
        .is_some_and(|id| id.eq_ignore_ascii_case("nightly"))
}

/// 从安装包名解析 nightly 版本。
/// 约定：`Shizi_{version}_x64-setup.exe`，version 形如 `0.7.0-nightly.YYYYMMDD.sha`。
pub fn extract_nightly_version_from_asset_name(name: &str) -> Option<semver::Version> {
    let base = name
        .strip_prefix("Shizi_")
        .or_else(|| name.strip_prefix("shizi_"))?;
    // version 段不含 `_`；其后为 arch-setup…
    let version_part = base.split('_').next()?;
    let version = parse_tag_version(version_part)?;
    if version_has_nightly_pre(&version) {
        Some(version)
    } else {
        None
    }
}

fn extract_nightly_version_from_assets(assets: &[ReleaseAsset]) -> Option<semver::Version> {
    assets
        .iter()
        .filter_map(|a| extract_nightly_version_from_asset_name(&a.name))
        .max()
}

/// 解析 Release 在指定通道下的候选版本。
/// - stable：仅无 pre、且非 GitHub prerelease 的 semver tag
/// - nightly：滚动 tag `nightly`（版本从资产名解析）或 tag 本身为 `*-nightly.*`
fn candidate_version_for_channel(
    r: &ReleaseInfo,
    channel: UpdateChannel,
) -> Option<(semver::Version, bool)> {
    match channel {
        UpdateChannel::Stable => {
            let version = parse_tag_version(&r.tag_name)?;
            let has_semver_pre = !version.pre.is_empty();
            if r.prerelease || has_semver_pre {
                return None;
            }
            Some((version, false))
        }
        UpdateChannel::Nightly => {
            if let Some(version) = parse_tag_version(&r.tag_name) {
                if version_has_nightly_pre(&version) {
                    return Some((version, true));
                }
                // 正式 / beta 等其它 pre 不进每日构建通道
                return None;
            }
            // CI 滚动 Release：tag 固定为 `nightly`（非 semver）
            if r.tag_name.trim().eq_ignore_ascii_case("nightly") {
                let version = extract_nightly_version_from_assets(&r.assets)?;
                return Some((version, true));
            }
            None
        }
    }
}

pub fn select_latest_for_channel(
    releases: &[ReleaseInfo],
    channel: UpdateChannel,
) -> Option<SelectedRelease> {
    releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let (version, is_prerelease) = candidate_version_for_channel(r, channel)?;
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
        .map(|v| version_has_nightly_pre(&v))
        .unwrap_or(false)
}

/// 通道内是否应提示可更新。
/// - stable：纯 semver 比较
/// - nightly：当前已是 nightly 时用 semver；当前是正式/其它版本时，只要有候选 nightly 即提示切换
fn should_offer_update(current_version: &str, latest: &str, channel: UpdateChannel) -> bool {
    match channel {
        UpdateChannel::Stable => is_update_available(current_version, latest),
        UpdateChannel::Nightly => {
            if is_nightly_build(current_version) {
                is_update_available(current_version, latest)
            } else {
                // 正式版用户改选每日构建：semver 上 nightly 常 < 同号正式版，不能用 > 比较
                true
            }
        }
    }
}

pub fn evaluate_check(
    current_version: &str,
    releases: &[ReleaseInfo],
    channel: UpdateChannel,
) -> CheckUpdateResult {
    // 运行中的是每日构建，但通道仍为正式版：避免 semver 误报「可升级到同号正式版」。
    // 跟随每日构建需把通道改为 nightly。
    if channel == UpdateChannel::Stable && is_nightly_build(current_version) {
        return CheckUpdateResult {
            status: "up_to_date".into(),
            current_version: current_version.to_string(),
            latest_version: None,
            release_name: None,
            release_url: Some(RELEASES_PAGE_FALLBACK.to_string()),
            is_prerelease: Some(true),
            message: Some(
                "当前为每日构建：更新通道为「正式版」时不会提示升级。若要跟随每日构建，请将通道改为「每日构建」。"
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
    if should_offer_update(current_version, &latest_str, channel) {
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
            release(
                "v0.8.0-nightly.20260801.abc",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0-nightly.20260801.abc",
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
    fn nightly_channel_picks_rolling_tag_from_asset_version() {
        let slim = "https://github.com/XuDaojie/shizi-translator/releases/download/nightly/Shizi_0.8.0-nightly.20260803.abc_x64-setup.exe";
        let releases = vec![
            release(
                "v0.8.0",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0",
                false,
                false,
                vec![],
            ),
            release(
                "v0.9.0-beta.1",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.9.0-beta.1",
                true,
                false,
                vec![],
            ),
            ReleaseInfo {
                tag_name: "nightly".into(),
                name: Some("Nightly 2026-08-03 (abc)".into()),
                html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/nightly"
                    .into(),
                prerelease: true,
                draft: false,
                assets: vec![asset(
                    "Shizi_0.8.0-nightly.20260803.abc_x64-setup.exe",
                    slim,
                )],
            },
        ];
        let latest = select_latest_for_channel(&releases, UpdateChannel::Nightly).unwrap();
        assert_eq!(latest.version.to_string(), "0.8.0-nightly.20260803.abc");
        assert!(latest.is_prerelease);
        assert_eq!(latest.download_url, slim);
    }

    #[test]
    fn nightly_channel_ignores_beta_and_stable_only() {
        let releases = vec![
            release(
                "v0.8.0",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0",
                false,
                false,
                vec![],
            ),
            release(
                "v0.9.0-beta.1",
                "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.9.0-beta.1",
                true,
                false,
                vec![],
            ),
        ];
        assert!(select_latest_for_channel(&releases, UpdateChannel::Nightly).is_none());
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
        // 两个 nightly 按 pre 段日期可比
        assert!(is_update_available(
            "0.7.0-nightly.20260721.abc",
            "0.7.0-nightly.20260722.def"
        ));
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
    fn extract_nightly_version_from_asset_name_parses() {
        assert_eq!(
            extract_nightly_version_from_asset_name(
                "Shizi_0.7.0-nightly.20260721.813a439_x64-setup.exe"
            )
            .unwrap()
            .to_string(),
            "0.7.0-nightly.20260721.813a439"
        );
        assert!(
            extract_nightly_version_from_asset_name("Shizi_0.7.0_x64-setup.exe").is_none()
        );
    }

    #[test]
    fn evaluate_check_stable_channel_skips_prompt_for_nightly_current() {
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
            .is_some_and(|m| m.contains("每日构建")));
        // 若未拦截，semver 会判定 0.7.0 > nightly 从而误报
        assert!(is_update_available(
            "0.7.0-nightly.20260721.813a439",
            "0.7.0"
        ));
    }

    #[test]
    fn evaluate_check_nightly_channel_offers_newer_nightly() {
        let slim = "https://github.com/XuDaojie/shizi-translator/releases/download/nightly/Shizi_0.7.0-nightly.20260722.def_x64-setup.exe";
        let releases = vec![ReleaseInfo {
            tag_name: "nightly".into(),
            name: Some("Nightly".into()),
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/nightly".into(),
            prerelease: true,
            draft: false,
            assets: vec![asset(
                "Shizi_0.7.0-nightly.20260722.def_x64-setup.exe",
                slim,
            )],
        }];
        let result = evaluate_check(
            "0.7.0-nightly.20260721.abc",
            &releases,
            UpdateChannel::Nightly,
        );
        assert_eq!(result.status, "update_available");
        assert_eq!(
            result.latest_version.as_deref(),
            Some("0.7.0-nightly.20260722.def")
        );
        assert_eq!(result.release_url.as_deref(), Some(slim));
    }

    #[test]
    fn evaluate_check_nightly_channel_offers_switch_from_stable() {
        // 同号正式版 semver > nightly，但仍应提示切换到每日构建
        let slim = "https://github.com/XuDaojie/shizi-translator/releases/download/nightly/Shizi_0.8.0-nightly.20260803.abc_x64-setup.exe";
        let releases = vec![ReleaseInfo {
            tag_name: "nightly".into(),
            name: Some("Nightly".into()),
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/nightly".into(),
            prerelease: true,
            draft: false,
            assets: vec![asset(
                "Shizi_0.8.0-nightly.20260803.abc_x64-setup.exe",
                slim,
            )],
        }];
        let result = evaluate_check("0.8.0", &releases, UpdateChannel::Nightly);
        assert_eq!(result.status, "update_available");
        assert_eq!(
            result.latest_version.as_deref(),
            Some("0.8.0-nightly.20260803.abc")
        );
        // 纯 semver 不会认为有更新
        assert!(!is_update_available("0.8.0", "0.8.0-nightly.20260803.abc"));
    }

    #[test]
    fn evaluate_check_nightly_channel_up_to_date_same_build() {
        let slim = "https://github.com/XuDaojie/shizi-translator/releases/download/nightly/Shizi_0.7.0-nightly.20260721.abc_x64-setup.exe";
        let releases = vec![ReleaseInfo {
            tag_name: "nightly".into(),
            name: Some("Nightly".into()),
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/nightly".into(),
            prerelease: true,
            draft: false,
            assets: vec![asset(
                "Shizi_0.7.0-nightly.20260721.abc_x64-setup.exe",
                slim,
            )],
        }];
        let result = evaluate_check(
            "0.7.0-nightly.20260721.abc",
            &releases,
            UpdateChannel::Nightly,
        );
        assert_eq!(result.status, "up_to_date");
        assert_eq!(
            result.latest_version.as_deref(),
            Some("0.7.0-nightly.20260721.abc")
        );
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

    #[test]
    fn update_channel_parse_migrates_beta_to_nightly() {
        assert_eq!(UpdateChannel::parse("nightly"), UpdateChannel::Nightly);
        assert_eq!(UpdateChannel::parse("beta"), UpdateChannel::Nightly);
        assert_eq!(UpdateChannel::parse("stable"), UpdateChannel::Stable);
        assert_eq!(UpdateChannel::parse("other"), UpdateChannel::Stable);
        assert_eq!(UpdateChannel::Nightly.as_str(), "nightly");
    }
}

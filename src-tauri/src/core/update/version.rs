use super::github::RELEASES_PAGE_FALLBACK;
use super::types::{CheckUpdateResult, ReleaseInfo, SelectedRelease, UpdateChannel};

/// `html_url` 须为 https；否则回退到 releases 列表页。
fn resolve_release_url(html_url: &str) -> String {
    if html_url.starts_with("https://") {
        html_url.to_string()
    } else {
        RELEASES_PAGE_FALLBACK.to_string()
    }
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
                html_url: r.html_url.clone(),
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

pub fn evaluate_check(
    current_version: &str,
    releases: &[ReleaseInfo],
    channel: UpdateChannel,
) -> CheckUpdateResult {
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
    let release_url = Some(resolve_release_url(&latest.html_url));
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
    fn stable_channel_skips_prerelease_flag_and_semver_pre() {
        let releases = vec![
            ReleaseInfo {
                tag_name: "v0.8.0-beta.1".into(),
                name: Some("beta".into()),
                html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0-beta.1"
                    .into(),
                prerelease: true,
                draft: false,
            },
            ReleaseInfo {
                tag_name: "v0.7.1".into(),
                name: Some("stable".into()),
                html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.1".into(),
                prerelease: false,
                draft: false,
            },
            ReleaseInfo {
                tag_name: "bad-tag".into(),
                name: None,
                html_url: "https://github.com/example/x".into(),
                prerelease: false,
                draft: false,
            },
        ];
        let latest = select_latest_for_channel(&releases, UpdateChannel::Stable).unwrap();
        assert_eq!(latest.version.to_string(), "0.7.1");
        assert!(!latest.is_prerelease);
    }

    #[test]
    fn beta_channel_picks_highest_including_prerelease() {
        let releases = vec![
            ReleaseInfo {
                tag_name: "v0.7.0".into(),
                name: None,
                html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.0".into(),
                prerelease: false,
                draft: false,
            },
            ReleaseInfo {
                tag_name: "v0.7.0-beta.9".into(),
                name: None,
                html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.0-beta.9"
                    .into(),
                prerelease: true,
                draft: false,
            },
            ReleaseInfo {
                tag_name: "v0.8.0-beta.1".into(),
                name: None,
                html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0-beta.1"
                    .into(),
                prerelease: true,
                draft: false,
            },
        ];
        // semver: 0.8.0-beta.1 > 0.7.0 > 0.7.0-beta.9
        let latest = select_latest_for_channel(&releases, UpdateChannel::Beta).unwrap();
        assert_eq!(latest.version.to_string(), "0.8.0-beta.1");
        assert!(latest.is_prerelease);
    }

    #[test]
    fn compare_update_available_when_latest_greater() {
        assert!(is_update_available("0.7.0", "0.7.1"));
        assert!(is_update_available("0.7.0", "0.7.0-beta.6") == false); // 0.7.0 > 0.7.0-beta.6
        assert!(!is_update_available("0.7.0", "0.7.0"));
        assert!(is_update_available("0.7.0-beta.5", "0.7.0-beta.6"));
    }

    #[test]
    fn draft_releases_are_ignored() {
        let releases = vec![ReleaseInfo {
            tag_name: "v9.0.0".into(),
            name: None,
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v9.0.0".into(),
            prerelease: false,
            draft: true,
        }];
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
        }];
        let result = evaluate_check("0.1.0", &releases, UpdateChannel::Stable);
        assert_eq!(result.status, "update_available");
        assert_eq!(
            result.release_url.as_deref(),
            Some(RELEASES_PAGE_FALLBACK)
        );
    }
}

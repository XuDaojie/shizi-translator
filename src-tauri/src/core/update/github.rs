use super::types::ReleaseInfo;

pub const GITHUB_OWNER_REPO: &str = "XuDaojie/shizi-translator";
pub const RELEASES_API_URL: &str =
    "https://api.github.com/repos/XuDaojie/shizi-translator/releases?per_page=30";
pub const RELEASES_PAGE_FALLBACK: &str =
    "https://github.com/XuDaojie/shizi-translator/releases";
const REQUEST_TIMEOUT_SECS: u64 = 12;

pub async fn fetch_releases(client: &reqwest::Client) -> Result<Vec<ReleaseInfo>, String> {
    let version = env!("CARGO_PKG_VERSION");
    let resp = client
        .get(RELEASES_API_URL)
        .header("User-Agent", format!("shizi/{version}"))
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| map_network_error(&e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(map_http_status(status.as_u16()));
    }

    resp.json::<Vec<ReleaseInfo>>()
        .await
        .map_err(|_| "无法解析 GitHub 响应".to_string())
}

fn map_http_status(code: u16) -> String {
    match code {
        403 | 429 => "GitHub 请求过于频繁或被拒绝，请稍后重试".into(),
        404 => "未找到发布信息".into(),
        s if s >= 500 => "GitHub 服务暂时不可用，请稍后重试".into(),
        _ => format!("检查更新失败（HTTP {code}）"),
    }
}

fn map_network_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "检查更新超时，请检查网络后重试".into()
    } else {
        "网络不可用，无法检查更新".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_http_status_rate_limit_and_forbidden() {
        assert_eq!(
            map_http_status(403),
            "GitHub 请求过于频繁或被拒绝，请稍后重试"
        );
        assert_eq!(
            map_http_status(429),
            "GitHub 请求过于频繁或被拒绝，请稍后重试"
        );
    }

    #[test]
    fn map_http_status_not_found() {
        assert_eq!(map_http_status(404), "未找到发布信息");
    }

    #[test]
    fn map_http_status_server_error() {
        assert_eq!(
            map_http_status(500),
            "GitHub 服务暂时不可用，请稍后重试"
        );
        assert_eq!(
            map_http_status(503),
            "GitHub 服务暂时不可用，请稍后重试"
        );
    }

    #[test]
    fn map_http_status_other() {
        assert_eq!(map_http_status(418), "检查更新失败（HTTP 418）");
        assert_eq!(map_http_status(400), "检查更新失败（HTTP 400）");
    }
}

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::core::mt::{EdgeTranslateEnv, DEFAULT_EDGE_USER_AGENT};
use crate::core::translation::provider::{
    BatchTranslateProvider, TranslationError, TranslationResult,
};
use crate::core::translation::TranslationRequest;

const EDGE_TRANSLATE_URL: &str = "https://edge.microsoft.com/translate/translatetext";

pub struct MicrosoftMtProvider {
    client: reqwest::Client,
    env: EdgeTranslateEnv,
}

impl MicrosoftMtProvider {
    pub fn new(env: EdgeTranslateEnv) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("创建 HTTP client 失败");
        Self { client, env }
    }

    fn effective_ua(&self) -> &str {
        if self.env.user_agent.trim().is_empty() {
            DEFAULT_EDGE_USER_AGENT
        } else {
            &self.env.user_agent
        }
    }
}

// ── 语言映射 ──────────────────────────────────────────────
// 内部 code（与前端 LANGUAGES 同源）↔ Edge code。
fn map_source_lang(internal: &str) -> Option<&'static str> {
    match internal {
        "auto" => None, // 省略 from，自动检测
        "zh-CN" => Some("zh-Hans"),
        "zh-TW" => Some("zh-Hant"),
        "en-US" => Some("en"),
        "ja-JP" => Some("ja"),
        "ko-KR" => Some("ko"),
        "fr-FR" => Some("fr"),
        "de-DE" => Some("de"),
        "es-ES" => Some("es"),
        "ru-RU" => Some("ru"),
        _ => None, // 未知语言省略 from（交由 Edge 自动检测），不阻断翻译
    }
}

fn map_target_lang(internal: &str) -> &str {
    match internal {
        "zh-CN" => "zh-Hans",
        "zh-TW" => "zh-Hant",
        "en-US" => "en",
        "ja-JP" => "ja",
        "ko-KR" => "ko",
        "fr-FR" => "fr",
        "de-DE" => "de",
        "es-ES" => "es",
        "ru-RU" => "ru",
        _ => "en", // 未知目标语言兜底英语
    }
}

/// Edge detectedLanguage.language（如 "en"）反向映射回内部 code（如 "en-US"）。
fn detected_to_internal(edge: &str) -> String {
    match edge {
        "zh-Hans" => "zh-CN".to_string(),
        "zh-Hant" => "zh-TW".to_string(),
        "en" => "en-US".to_string(),
        "ja" => "ja-JP".to_string(),
        "ko" => "ko-KR".to_string(),
        "fr" => "fr-FR".to_string(),
        "de" => "de-DE".to_string(),
        "es" => "es-ES".to_string(),
        "ru" => "ru-RU".to_string(),
        other => other.to_string(),
    }
}

// ── UA 解析 ───────────────────────────────────────────────
struct EdgeHeaders {
    edge_version: String,
    os_version: String,
    arch: String,
    sec_ch_ua: String,
}

/// 从 UA 解析派生 sec-mesh-client-* / sec-ch-ua 头所需字段。纯函数，单测覆盖。
fn parse_edge_headers(ua: &str) -> EdgeHeaders {
    let edge_version = extract_token(ua, "Edg/").unwrap_or_default();
    let chrome_version = extract_token(ua, "Chrome/").unwrap_or_default();
    let os_version = extract_paren_token(ua, "Windows NT ").unwrap_or_default();
    let arch = if ua.contains("Win64; x64") {
        "x86_64".to_string()
    } else if ua.contains("WoW64") {
        "x86_64".to_string()
    } else {
        String::new()
    };
    let sec_ch_ua = if !edge_version.is_empty() {
        let v = major(&chrome_version);
        let ve = major(&edge_version);
        format!(
            "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"{}\", \"Microsoft Edge\";v=\"{}\"",
            v, ve
        )
    } else {
        String::new()
    };
    EdgeHeaders {
        edge_version,
        os_version,
        arch,
        sec_ch_ua,
    }
}

/// 取 `Edg/` / `Chrome/` 后到首个非版本字符（空格/`)`）的子串。
fn extract_token(ua: &str, prefix: &str) -> Option<String> {
    let start = ua.find(prefix)? + prefix.len();
    let rest = &ua[start..];
    let end = rest
        .find(|c: char| c == ' ' || c == ')')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// 取 `Windows NT ` 后到 `)` 或 `;` 的子串。
fn extract_paren_token(ua: &str, prefix: &str) -> Option<String> {
    let start = ua.find(prefix)? + prefix.len();
    let rest = &ua[start..];
    let end = rest
        .find(|c: char| c == ';' || c == ')')
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

fn major(version: &str) -> &str {
    version.split('.').next().unwrap_or(version)
}

// ── 请求拼装 ──────────────────────────────────────────────
fn build_url(from: Option<&str>, to: &str) -> String {
    let mut url = format!(
        "{}?to={}&isEnterpriseClient=false",
        EDGE_TRANSLATE_URL, to
    );
    if let Some(from) = from {
        url.push_str(&format!("&from={}", from));
    }
    url
}

fn build_headers(env: &EdgeTranslateEnv, ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let put = |headers: &mut HeaderMap, name: &str, value: &str| {
        if let (Ok(n), Ok(v)) = (
            HeaderName::try_from(name),
            HeaderValue::from_str(value),
        ) {
            headers.insert(n, v);
        }
    };
    // 常量头
    put(&mut headers, "accept", "*/*");
    put(&mut headers, "content-type", "application/json");
    put(&mut headers, "origin", "https://github.com");
    put(&mut headers, "referer", "https://github.com/");
    put(&mut headers, "priority", "u=1, i");
    put(&mut headers, "sec-fetch-dest", "empty");
    put(&mut headers, "sec-fetch-mode", "cors");
    put(&mut headers, "sec-fetch-site", "cross-site");
    put(&mut headers, "sec-ch-ua-platform", "\"Windows\"");
    put(&mut headers, "sec-ch-ua-mobile", "?0");
    put(&mut headers, "sec-mesh-client-os", "Windows");
    put(&mut headers, "sec-mesh-client-edge-channel", "stable");
    put(&mut headers, "sec-mesh-client-webview", "0");
    put(&mut headers, "x-edge-shopping-flag", "0");
    // 来自 env
    put(&mut headers, "user-agent", ua);
    let accept_language = if env.accept_language.trim().is_empty() {
        "zh-CN,zh;q=0.9,en;q=0.8"
    } else {
        env.accept_language.as_str()
    };
    put(&mut headers, "accept-language", accept_language);
    // 从 UA 派生
    let derived = parse_edge_headers(ua);
    if !derived.edge_version.is_empty() {
        put(&mut headers, "sec-mesh-client-edge-version", &derived.edge_version);
    }
    if !derived.os_version.is_empty() {
        put(&mut headers, "sec-mesh-client-os-version", &derived.os_version);
    }
    if !derived.arch.is_empty() {
        put(&mut headers, "sec-mesh-client-arch", &derived.arch);
    }
    if !derived.sec_ch_ua.is_empty() {
        put(&mut headers, "sec-ch-ua", &derived.sec_ch_ua);
    }
    headers
}

// ── 响应解析 ──────────────────────────────────────────────
// 基于通用结构（Azure Translator 同族字段名 translations/detectedLanguage）。
// 若 Edge 端点实测响应结构不同，按真实响应调整反序列化结构体（spec 5.4 不锁定字段名）。
#[derive(Deserialize)]
struct EdgeTranslation {
    translations: Vec<EdgeTranslationText>,
    #[serde(rename = "detectedLanguage")]
    detected_language: Option<EdgeDetectedLanguage>,
}

#[derive(Deserialize)]
struct EdgeTranslationText {
    text: String,
}

#[derive(Deserialize)]
struct EdgeDetectedLanguage {
    language: String,
}

#[async_trait::async_trait]
impl BatchTranslateProvider for MicrosoftMtProvider {
    async fn translate_once(
        &self,
        request: &TranslationRequest,
        cancel: &CancellationToken,
    ) -> Result<TranslationResult, TranslationError> {
        let text = request.source_text();
        let from = map_source_lang(&request.source_lang);
        let to = map_target_lang(&request.target_lang);
        let url = build_url(from, to);
        let ua = self.effective_ua().to_string();
        let headers = build_headers(&self.env, &ua);
        let body =
            serde_json::to_string(&[text]).map_err(|e| TranslationError::Parse(e.to_string()))?;

        let req = self.client.post(&url).body(body).headers(headers);

        let resp = tokio::select! {
            _ = cancel.cancelled() => return Ok(TranslationResult::default()),
            r = req.send() => r.map_err(|e| TranslationError::Http(e.to_string()))?,
        };

        let status = resp.status();
        if !status.is_success() {
            let retryable = status.as_u16() == 429 || status.is_server_error();
            let body = resp.text().await.unwrap_or_default();
            let message = format!(
                "HTTP {}: {}",
                status,
                body.chars().take(500).collect::<String>()
            );
            log::warn!(
                "Edge 翻译响应非 2xx: status={} retryable={}",
                status,
                retryable
            );
            return if retryable {
                Err(TranslationError::Http(message))
            } else {
                Err(TranslationError::Api {
                    message,
                    retryable: false,
                })
            };
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| TranslationError::Http(e.to_string()))?;
        let parsed: Vec<EdgeTranslation> = serde_json::from_slice(&bytes)
            .map_err(|e| TranslationError::Parse(e.to_string()))?;
        let first = parsed
            .into_iter()
            .next()
            .ok_or_else(|| TranslationError::Parse("响应数组为空".to_string()))?;

        let detected = if request.source_lang == "auto" {
            first
                .detected_language
                .map(|d| detected_to_internal(&d.language))
        } else {
            None
        };
        let translated = first
            .translations
            .into_iter()
            .next()
            .map(|t| t.text)
            .unwrap_or_default();

        Ok(TranslationResult {
            text: translated,
            usage: None,
            detected_source_lang: detected,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_edge_headers_extracts_versions_from_real_ua() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 Edg/150.0.0.0";
        let h = parse_edge_headers(ua);
        assert_eq!(h.edge_version, "150.0.0.0");
        assert_eq!(h.os_version, "10.0");
        assert_eq!(h.arch, "x86_64");
        assert!(
            h.sec_ch_ua.contains("\"Chromium\";v=\"150\""),
            "sec-ch-ua 应含 Chromium 150: {}",
            h.sec_ch_ua
        );
        assert!(
            h.sec_ch_ua.contains("\"Microsoft Edge\";v=\"150\""),
            "sec-ch-ua 应含 Edge 150: {}",
            h.sec_ch_ua
        );
    }

    #[test]
    fn parse_edge_headers_fallbacks_when_fields_missing() {
        let h = parse_edge_headers("some unknown ua string");
        assert!(h.edge_version.is_empty());
        assert!(h.os_version.is_empty());
        assert!(h.arch.is_empty());
        assert!(h.sec_ch_ua.is_empty());
    }

    #[test]
    fn map_source_lang_auto_is_none() {
        assert_eq!(map_source_lang("auto"), None);
    }
    #[test]
    fn map_source_lang_known() {
        assert_eq!(map_source_lang("zh-CN"), Some("zh-Hans"));
        assert_eq!(map_source_lang("en-US"), Some("en"));
    }
    #[test]
    fn map_target_lang_known_and_fallback() {
        assert_eq!(map_target_lang("ja-JP"), "ja");
        assert_eq!(map_target_lang("unknown"), "en");
    }
    #[test]
    fn detected_to_internal_roundtrip() {
        assert_eq!(detected_to_internal("en"), "en-US");
        assert_eq!(detected_to_internal("zh-Hans"), "zh-CN");
        assert_eq!(detected_to_internal("fr"), "fr-FR");
    }
    #[test]
    fn build_url_omits_from_for_auto() {
        assert_eq!(
            build_url(None, "zh-Hans"),
            "https://edge.microsoft.com/translate/translatetext?to=zh-Hans&isEnterpriseClient=false"
        );
        assert_eq!(
            build_url(Some("en"), "zh-Hans"),
            "https://edge.microsoft.com/translate/translatetext?to=zh-Hans&isEnterpriseClient=false&from=en"
        );
    }
    #[test]
    fn build_headers_includes_env_and_derived() {
        let env = EdgeTranslateEnv {
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/150.0.0.0 Edg/150.0.0.0"
                .to_string(),
            accept_language: "zh-CN,zh;q=0.9".to_string(),
        };
        let h = build_headers(&env, &env.user_agent);
        assert_eq!(h.get("user-agent").unwrap(), &env.user_agent);
        assert_eq!(h.get("accept-language").unwrap(), "zh-CN,zh;q=0.9");
        assert_eq!(
            h.get("sec-mesh-client-edge-version").unwrap(),
            "150.0.0.0"
        );
        assert_eq!(h.get("sec-mesh-client-arch").unwrap(), "x86_64");
        assert!(h
            .get("sec-ch-ua")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Microsoft Edge"));
    }
    #[test]
    fn build_headers_uses_default_ua_when_env_empty() {
        let env = EdgeTranslateEnv::default();
        let h = build_headers(&env, DEFAULT_EDGE_USER_AGENT);
        assert_eq!(h.get("user-agent").unwrap(), DEFAULT_EDGE_USER_AGENT);
    }

    // 响应解析：构造离线 fixture，不联网
    #[test]
    fn parse_response_extracts_text_and_detected() {
        let json = r#"[{"translations":[{"text":"你好","to":"zh-Hans"}],"detectedLanguage":{"language":"en","score":1.0}}]"#;
        let parsed: Vec<EdgeTranslation> = serde_json::from_str(json).unwrap();
        let first = parsed.into_iter().next().unwrap();
        assert_eq!(first.translations[0].text, "你好");
        assert_eq!(first.detected_language.unwrap().language, "en");
    }

    #[test]
    fn effective_ua_falls_back_to_default_when_empty() {
        let provider = MicrosoftMtProvider::new(EdgeTranslateEnv::default());
        assert_eq!(provider.effective_ua(), DEFAULT_EDGE_USER_AGENT);
    }
}

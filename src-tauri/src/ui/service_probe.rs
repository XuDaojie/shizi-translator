use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceProbeRequest {
    pub protocol: String,
    pub endpoint: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsResult {
    pub models: Vec<String>,
}

/// OpenAI 兼容 `/models` 响应：`{ "object":"list", "data":[{ "id":"..." }] }`
#[derive(Debug, Clone, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelItem {
    id: String,
}

impl ServiceProbeRequest {
    pub fn validate(&self) -> Result<(), String> {
        let protocol = self.protocol.trim();
        if protocol != "openai_chat" && protocol != "claude_messages" {
            return Err("当前协议不支持拉取模型（仅 openai_chat / claude_messages）".to_string());
        }
        let key = self.api_key.as_deref().unwrap_or("").trim();
        if key.is_empty() {
            return Err("请先填写 API Key".to_string());
        }
        let endpoint = self.endpoint.trim();
        if endpoint.is_empty() {
            return Err("请先填写 Endpoint".to_string());
        }
        let url = reqwest::Url::parse(endpoint)
            .map_err(|_| "Endpoint 请输入有效的 http(s) 地址".to_string())?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err("Endpoint 请输入有效的 http(s) 地址".to_string());
        }
        Ok(())
    }
}

/// 由 base endpoint 拼出 OpenAI 兼容的 `GET {base}/models`。
/// Claude 官方 base 常为 `https://api.anthropic.com`（无 /v1），需补到 `/v1/models`。
fn models_endpoint(endpoint: &str, protocol: &str) -> String {
    let base = endpoint.trim().trim_end_matches('/');
    if protocol.trim() == "claude_messages" {
        if base.ends_with("/v1") {
            return format!("{base}/models");
        }
        return format!("{base}/v1/models");
    }
    format!("{base}/models")
}

fn extract_model_ids(body: ModelsResponse) -> Vec<String> {
    let mut models: Vec<String> = body
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| !id.trim().is_empty())
        .collect();
    models.sort();
    models.dedup();
    models
}

#[tauri::command]
pub async fn validate_service_credential(request: ServiceProbeRequest) -> Result<(), String> {
    let _ = list_service_models(request).await?;
    Ok(())
}

#[tauri::command]
pub async fn list_service_models(request: ServiceProbeRequest) -> Result<ModelsResult, String> {
    request.validate()?;
    let api_key = request.api_key.as_deref().unwrap_or("").trim();
    let protocol = request.protocol.trim();
    let url = models_endpoint(&request.endpoint, protocol);

    log::info!(
        "拉取模型列表: protocol={} url={}",
        protocol,
        url
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let mut builder = client.get(&url);
    builder = match protocol {
        "claude_messages" => builder
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01"),
        // OpenAI Chat 兼容（DeepSeek / 智谱 / Moonshot / 硅基流动 / Gemini OpenAI 兼容等）
        _ => builder.bearer_auth(api_key),
    };

    let response = builder
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;

    if !status.is_success() {
        let snippet: String = text.chars().take(300).collect();
        log::warn!(
            "拉取模型 HTTP 失败: status={} body={}",
            status,
            snippet
        );
        // 尝试从 JSON error.message 里拿更友好的文案
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(msg) = v
                .pointer("/error/message")
                .and_then(|m| m.as_str())
                .map(str::trim)
                .filter(|m| !m.is_empty())
            {
                return Err(format!("服务返回 HTTP {status}: {msg}"));
            }
        }
        return Err(if snippet.is_empty() {
            format!("服务返回 HTTP {status}")
        } else {
            format!("服务返回 HTTP {status}: {snippet}")
        });
    }

    let body: ModelsResponse = serde_json::from_str(&text).map_err(|e| {
        let snippet: String = text.chars().take(200).collect();
        format!("模型列表解析失败: {e}；响应片段: {snippet}")
    })?;

    let models = extract_model_ids(body);
    log::info!("拉取模型列表成功: count={}", models.len());
    Ok(ModelsResult { models })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_endpoint_trims_trailing_slash() {
        assert_eq!(
            models_endpoint("https://api.example.com/v1/", "openai_chat"),
            "https://api.example.com/v1/models"
        );
    }

    #[test]
    fn models_endpoint_preserves_without_slash() {
        assert_eq!(
            models_endpoint("https://api.example.com/v1", "openai_chat"),
            "https://api.example.com/v1/models"
        );
    }

    #[test]
    fn models_endpoint_works_with_root() {
        // DeepSeek: https://api.deepseek.com/models
        assert_eq!(
            models_endpoint("https://api.deepseek.com", "openai_chat"),
            "https://api.deepseek.com/models"
        );
    }

    #[test]
    fn models_endpoint_claude_adds_v1() {
        assert_eq!(
            models_endpoint("https://api.anthropic.com", "claude_messages"),
            "https://api.anthropic.com/v1/models"
        );
        assert_eq!(
            models_endpoint("https://api.anthropic.com/v1", "claude_messages"),
            "https://api.anthropic.com/v1/models"
        );
    }

    #[test]
    fn extract_model_ids_sorts_and_dedups() {
        let body = ModelsResponse {
            data: vec![
                ModelItem {
                    id: "b".to_string(),
                },
                ModelItem {
                    id: "a".to_string(),
                },
                ModelItem {
                    id: "a".to_string(),
                },
                ModelItem {
                    id: "  ".to_string(),
                },
            ],
        };
        assert_eq!(extract_model_ids(body), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn probe_validate_rejects_invalid_protocol() {
        let req = ServiceProbeRequest {
            protocol: "unknown_protocol".to_string(),
            endpoint: "https://api.example.com/v1".to_string(),
            api_key: Some("sk-test".to_string()),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn probe_validate_rejects_missing_key() {
        let req = ServiceProbeRequest {
            protocol: "openai_chat".to_string(),
            endpoint: "https://api.example.com/v1".to_string(),
            api_key: Some("  ".to_string()),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn probe_validate_rejects_empty_endpoint() {
        let req = ServiceProbeRequest {
            protocol: "openai_chat".to_string(),
            endpoint: "  ".to_string(),
            api_key: Some("sk-test".to_string()),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn probe_validate_rejects_invalid_url() {
        let req = ServiceProbeRequest {
            protocol: "openai_chat".to_string(),
            endpoint: "not-a-url".to_string(),
            api_key: Some("sk-test".to_string()),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn probe_validate_accepts_valid_request() {
        let req = ServiceProbeRequest {
            protocol: "openai_chat".to_string(),
            endpoint: "https://api.deepseek.com".to_string(),
            api_key: Some("sk-test".to_string()),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn models_response_parses_deepseek_shape() {
        let json = r#"{
            "object": "list",
            "data": [
                { "id": "deepseek-chat", "object": "model", "owned_by": "deepseek" },
                { "id": "deepseek-reasoner", "object": "model", "owned_by": "deepseek" }
            ]
        }"#;
        let body: ModelsResponse = serde_json::from_str(json).expect("parse");
        assert_eq!(
            extract_model_ids(body),
            vec![
                "deepseek-chat".to_string(),
                "deepseek-reasoner".to_string()
            ]
        );
    }
}

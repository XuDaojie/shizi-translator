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

#[derive(Debug, Clone, Deserialize)]
struct ModelsResponse {
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
            return Err("当前协议不可用".to_string());
        }
        let key = self.api_key.as_deref().unwrap_or("").trim();
        if key.is_empty() {
            return Err("请先填写 API Key".to_string());
        }
        let url = reqwest::Url::parse(self.endpoint.trim())
            .map_err(|_| "Endpoint 请输入有效的 http(s) 地址".to_string())?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err("Endpoint 请输入有效的 http(s) 地址".to_string());
        }
        Ok(())
    }
}

// ponytail: 纯函数, 方便单测
fn models_endpoint(endpoint: &str) -> String {
    format!("{}/models", endpoint.trim_end_matches('/'))
}

#[tauri::command]
pub async fn validate_service_credential(request: ServiceProbeRequest) -> Result<(), String> {
    let _ = list_service_models(request).await?;
    Ok(())
}

#[tauri::command]
pub async fn list_service_models(
    request: ServiceProbeRequest,
) -> Result<ModelsResult, String> {
    request.validate()?;
    let api_key = request.api_key.as_deref().unwrap_or("").trim();
    let client = reqwest::Client::new();
    let mut builder = client.get(models_endpoint(&request.endpoint));
    builder = match request.protocol.trim() {
        "claude_messages" => builder
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01"),
        _ => builder.bearer_auth(api_key),
    };

    let response = builder
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("服务返回 HTTP {}", response.status()));
    }
    let body = response
        .json::<ModelsResponse>()
        .await
        .map_err(|e| format!("模型列表解析失败: {e}"))?;
    Ok(ModelsResult {
        models: body
            .data
            .into_iter()
            .map(|m| m.id)
            .filter(|id| !id.trim().is_empty())
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_endpoint_trims_trailing_slash() {
        assert_eq!(
            models_endpoint("https://api.example.com/v1/"),
            "https://api.example.com/v1/models"
        );
    }

    #[test]
    fn models_endpoint_preserves_without_slash() {
        assert_eq!(
            models_endpoint("https://api.example.com/v1"),
            "https://api.example.com/v1/models"
        );
    }

    #[test]
    fn models_endpoint_works_with_root() {
        assert_eq!(
            models_endpoint("https://api.example.com"),
            "https://api.example.com/models"
        );
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
            endpoint: "https://api.example.com/v1".to_string(),
            api_key: Some("sk-test".to_string()),
        };
        assert!(req.validate().is_ok());
    }
}

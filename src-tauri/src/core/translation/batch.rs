use crate::core::{
    config::ServiceInstanceConfig,
    translation::{
        TranslationInput, TranslationRequest, TranslationServiceMeta, TranslationSessionId,
    },
};

pub fn build_batch_requests(
    input: TranslationInput,
    target_lang: String,
    services: &[ServiceInstanceConfig],
    batch_id: &str,
) -> Result<Vec<TranslationRequest>, String> {
    let requests: Vec<TranslationRequest> = services
        .iter()
        .filter(|s| s.enabled)
        .map(|s| TranslationRequest {
            session_id: TranslationSessionId(format!("{}:{}", batch_id, s.id)),
            input: input.clone(),
            target_lang: target_lang.clone(),
            service: TranslationServiceMeta {
                service_instance_id: s.id.clone(),
                service_name: s.name.clone(),
                service_type: s.service_type.clone(),
                protocol: s.protocol.clone(),
            },
        })
        .collect();

    if requests.is_empty() {
        Err("请先在服务列表启用至少一个已配置服务".to_string())
    } else {
        Ok(requests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service(id: &str, enabled: bool) -> ServiceInstanceConfig {
        ServiceInstanceConfig {
            id: id.to_string(),
            service_type: "deepseek".to_string(),
            name: format!("svc-{}", id),
            enabled,
            protocol: "openai_chat".to_string(),
            api_key: Some("sk-x".to_string()),
            endpoint: "https://api.deepseek.com".to_string(),
            model: "deepseek-chat".to_string(),
            timeout_seconds: 60,
        }
    }

    #[test]
    fn build_batch_keeps_enabled_order() {
        let input = TranslationInput::ManualText("hello".to_string());
        let requests = build_batch_requests(
            input,
            "中文".to_string(),
            &[service("a", true), service("b", false), service("c", true)],
            "batch-1",
        )
        .expect("应生成批次");

        assert_eq!(
            requests.iter().map(|r| r.session_id.0.as_str()).collect::<Vec<_>>(),
            vec!["batch-1:a", "batch-1:c"]
        );
        assert_eq!(
            requests.iter().map(|r| r.service.service_instance_id.as_str()).collect::<Vec<_>>(),
            vec!["a", "c"]
        );
        assert_eq!(requests[0].service.service_name, "svc-a");
    }

    #[test]
    fn build_batch_rejects_empty() {
        let err = build_batch_requests(
            TranslationInput::ManualText("hello".to_string()),
            "中文".to_string(),
            &[service("a", false)],
            "batch-1",
        )
        .expect_err("应报错");
        assert_eq!(err, "请先在服务列表启用至少一个已配置服务");
    }
}

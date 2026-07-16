use crate::core::config::types::OcrServiceInstanceConfig;

use super::OcrError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionOcrConfig {
    pub service_type: String,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    /// 空则引擎使用 DEFAULT_OCR_PROMPT
    pub ocr_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedOcrEngine {
    WindowsMedia,
    VisionOpenAiCompatible(VisionOcrConfig),
}

/// 从 ocr_services 解析唯一启用引擎。调用前配置应已 normalized；
/// 仍对脏配置兜底：0 → NoEngineConfigured；>1 → 取第一个 + log::warn。
pub fn resolve_ocr_engine(
    services: &[OcrServiceInstanceConfig],
) -> Result<ResolvedOcrEngine, OcrError> {
    resolve_ocr_engine_for(services, None)
}

/// OCR 引擎解析。
/// - `override_id = Some(id)`：按 id 查找实例，**不检查 enabled**；缺失 → UnknownService。
/// - `override_id = None`：仅 enabled（与历史行为一致）。
pub fn resolve_ocr_engine_for(
    services: &[OcrServiceInstanceConfig],
    override_id: Option<&str>,
) -> Result<ResolvedOcrEngine, OcrError> {
    if let Some(id) = override_id {
        let service = services
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| OcrError::UnknownService(id.to_string()))?;
        return map_service(service);
    }

    let enabled: Vec<&OcrServiceInstanceConfig> =
        services.iter().filter(|s| s.enabled).collect();

    if enabled.is_empty() {
        return Err(OcrError::NoEngineConfigured);
    }

    if enabled.len() > 1 {
        log::warn!(
            "多个 OCR 服务同时启用（{} 个），仅使用第一个：id={}",
            enabled.len(),
            enabled[0].id
        );
    }

    let service = enabled[0];
    map_service(service)
}

fn map_service(service: &OcrServiceInstanceConfig) -> Result<ResolvedOcrEngine, OcrError> {
    let ty = service.service_type.as_str();

    if ty == "windows-media-ocr" {
        return Ok(ResolvedOcrEngine::WindowsMedia);
    }

    if is_openai_compatible_vision(ty) {
        let api_key = service
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .ok_or_else(|| OcrError::Auth("请填写 API Key".into()))?;

        return Ok(ResolvedOcrEngine::VisionOpenAiCompatible(VisionOcrConfig {
            service_type: service.service_type.clone(),
            endpoint: service.endpoint.clone(),
            api_key: api_key.to_string(),
            model: service.model.clone(),
            ocr_prompt: service.ocr_prompt.clone(),
        }));
    }

    Err(OcrError::UnsupportedProtocol(service.service_type.clone()))
}

fn is_openai_compatible_vision(service_type: &str) -> bool {
    matches!(
        service_type,
        "openai-vision"
            | "gemini-vision"
            | "zhipu-vl"
            | "siliconflow-vision"
            | "moonshot-vision"
            | "openai-compatible-vision"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::types::OcrServiceInstanceConfig;

    fn svc(id: &str, ty: &str, enabled: bool, key: Option<&str>) -> OcrServiceInstanceConfig {
        OcrServiceInstanceConfig {
            id: id.into(),
            service_type: ty.into(),
            name: id.into(),
            enabled,
            api_key: key.map(|s| s.into()),
            endpoint: "https://api.openai.com/v1".into(),
            model: "gpt-4o".into(),
            preferred_lang: String::new(),
            ocr_prompt: "自定义".into(),
        }
    }

    #[test]
    fn resolve_windows_only() {
        let r = resolve_ocr_engine(&[svc("w", "windows-media-ocr", true, None)]).unwrap();
        assert_eq!(r, ResolvedOcrEngine::WindowsMedia);
    }

    #[test]
    fn resolve_openai_vision() {
        let r =
            resolve_ocr_engine(&[svc("v", "openai-vision", true, Some("sk-test"))]).unwrap();
        match r {
            ResolvedOcrEngine::VisionOpenAiCompatible(c) => {
                assert_eq!(c.model, "gpt-4o");
                assert_eq!(c.api_key, "sk-test");
                assert_eq!(c.ocr_prompt, "自定义");
                assert_eq!(c.service_type, "openai-vision");
            }
            _ => panic!("expected vision"),
        }
    }

    #[test]
    fn resolve_claude_unsupported() {
        let err =
            resolve_ocr_engine(&[svc("c", "claude-vision", true, Some("sk"))]).unwrap_err();
        assert!(matches!(err, OcrError::UnsupportedProtocol(_)));
    }

    #[test]
    fn resolve_multiple_enabled_takes_first() {
        let list = vec![
            svc("v", "openai-vision", true, Some("sk")),
            svc("w", "windows-media-ocr", true, None),
        ];
        let r = resolve_ocr_engine(&list).unwrap();
        assert!(matches!(r, ResolvedOcrEngine::VisionOpenAiCompatible(_)));
    }

    #[test]
    fn resolve_none_enabled_errors() {
        let err =
            resolve_ocr_engine(&[svc("w", "windows-media-ocr", false, None)]).unwrap_err();
        assert_eq!(err, OcrError::NoEngineConfigured);
    }

    #[test]
    fn resolve_vision_missing_key_is_auth() {
        let err = resolve_ocr_engine(&[svc("v", "openai-vision", true, None)]).unwrap_err();
        assert!(matches!(err, OcrError::Auth(_)));
    }

    #[test]
    fn resolve_for_by_id_ignores_enabled() {
        let list = vec![
            svc("w", "windows-media-ocr", true, None),
            svc("v", "openai-vision", false, Some("sk-test")),
        ];
        let r = resolve_ocr_engine_for(&list, Some("v")).unwrap();
        match r {
            ResolvedOcrEngine::VisionOpenAiCompatible(c) => {
                assert_eq!(c.api_key, "sk-test");
                assert_eq!(c.model, "gpt-4o");
            }
            _ => panic!("expected vision by id"),
        }
    }

    #[test]
    fn resolve_for_missing_id_is_unknown_service() {
        let list = vec![svc("w", "windows-media-ocr", true, None)];
        let err = resolve_ocr_engine_for(&list, Some("nope")).unwrap_err();
        assert!(matches!(err, OcrError::UnknownService(id) if id == "nope"));
    }

    #[test]
    fn resolve_for_none_uses_enabled_only() {
        let list = vec![
            svc("w", "windows-media-ocr", false, None),
            svc("v", "openai-vision", true, Some("sk")),
        ];
        let r = resolve_ocr_engine_for(&list, None).unwrap();
        assert!(matches!(r, ResolvedOcrEngine::VisionOpenAiCompatible(_)));
    }

    #[test]
    fn resolve_for_by_id_vision_missing_key_is_auth() {
        let list = vec![svc("v", "openai-vision", false, None)];
        let err = resolve_ocr_engine_for(&list, Some("v")).unwrap_err();
        assert!(matches!(err, OcrError::Auth(_)));
    }

    #[test]
    fn resolve_ocr_engine_delegates_to_for_none() {
        let list = vec![svc("w", "windows-media-ocr", true, None)];
        assert_eq!(
            resolve_ocr_engine(&list).unwrap(),
            resolve_ocr_engine_for(&list, None).unwrap()
        );
    }
}

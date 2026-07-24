//! 路径 R：卡片元信息纯函数（MT vs LLM 展示策略）。

/// Microsoft Edge 机翻协议：隐藏 model / tokens。
pub fn is_machine_translate_protocol(protocol: &str) -> bool {
    protocol.trim() == "microsoft_edge"
}

/// 展示用模型名；MT 恒为空（与 GDI `card_detail_label` 不同，后者对 MT 显示协议名）。
pub fn display_model_name(protocol: &str, model_name: &str) -> String {
    if is_machine_translate_protocol(protocol) {
        return String::new();
    }
    let m = model_name.trim();
    if m.is_empty() || m == "—" || m == "-" {
        String::new()
    } else {
        m.to_string()
    }
}

/// 是否展示 token 用量；MT 永不展示，LLM 仅在有 usage 时展示。
pub fn should_show_tokens(protocol: &str, has_usage: bool) -> bool {
    if is_machine_translate_protocol(protocol) {
        return false;
    }
    has_usage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mt_protocol_hides_model_and_tokens() {
        assert!(is_machine_translate_protocol("microsoft_edge"));
        assert_eq!(display_model_name("microsoft_edge", "anything"), "");
        assert!(!should_show_tokens("microsoft_edge", true));
    }

    #[test]
    fn llm_shows_model_and_tokens_when_usage() {
        assert_eq!(display_model_name("openai_chat", "gpt-4o"), "gpt-4o");
        assert!(should_show_tokens("openai_chat", true));
        assert!(!should_show_tokens("openai_chat", false));
    }

    #[test]
    fn display_model_name_treats_placeholder_as_empty() {
        assert_eq!(display_model_name("openai_chat", "—"), "");
        assert_eq!(display_model_name("openai_chat", "-"), "");
        assert_eq!(display_model_name("openai_chat", "  "), "");
    }
}

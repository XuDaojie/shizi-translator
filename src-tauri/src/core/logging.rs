//! 日志脱敏与等级归一化纯函数。无 Tauri 依赖，core 层可自由调用。

/// API Key 脱敏：前 4 + `...` + 后 4。短于 9 字符（含 8）全遮蔽 `****`。
/// 空字符串也返回 `****`。
pub fn redact_api_key(key: &str) -> String {
    let len = key.chars().count();
    if len < 9 {
        return "****".to_string();
    }
    let head: String = key.chars().take(4).collect();
    let tail: String = key.chars().skip(len - 4).collect();
    format!("{head}...{tail}")
}

/// 翻译正文脱敏：`info` 及以上记摘要（`[len=N] 前20字...`），`debug` 记原文。
/// `level` 非 `debug` 时一律按摘要处理（与 `normalize_log_level` 的回退 `info` 一致）。
pub fn redact_text(text: &dyn std::fmt::Display, level: &str) -> String {
    let full = text.to_string();
    if level == "debug" {
        return full;
    }
    let len = full.chars().count();
    let head: String = full.chars().take(20).collect();
    format!("[len={len}] {head}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_api_key_keeps_first4_and_last4() {
        assert_eq!(redact_api_key("sk-abcdef12345678"), "sk-a...5678");
    }

    #[test]
    fn redact_api_key_masks_short_key_fully() {
        assert_eq!(redact_api_key("short"), "****");
        assert_eq!(redact_api_key("1234567"), "****");
    }

    #[test]
    fn redact_api_key_masks_exactly_8_chars() {
        // 等于 8 字符：前 4 + 后 4 会重叠，按短于规则全遮蔽
        assert_eq!(redact_api_key("12345678"), "****");
    }

    #[test]
    fn redact_api_key_handles_9_chars() {
        assert_eq!(redact_api_key("123456789"), "1234...6789");
    }

    #[test]
    fn redact_api_key_handles_none() {
        assert_eq!(redact_api_key(""), "****");
    }

    #[test]
    fn redact_text_info_level_returns_summary() {
        let text = "Hello, this is a long translation text.";
        let redacted = redact_text(&text, "info");
        assert!(redacted.starts_with("[len=39]"));
        assert!(redacted.contains("Hello, this is a lon"));
        assert!(redacted.ends_with("..."));
        assert!(!redacted.contains("translation text."));
    }

    #[test]
    fn redact_text_debug_level_returns_full() {
        let text = "Hello, this is a long translation text.";
        assert_eq!(redact_text(&text, "debug"), text);
    }

    #[test]
    fn redact_text_info_short_text_includes_full_head() {
        let text = "短文本";
        let redacted = redact_text(&text, "info");
        assert!(redacted.starts_with("[len=3]"));
        assert!(redacted.contains("短文本"));
    }

    #[test]
    fn redact_text_non_string_normalizes() {
        let redacted = redact_text(&42u32, "info");
        assert!(redacted.starts_with("[len=2]"));
    }
}

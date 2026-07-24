//! 路径 R：语言表与交换纯函数（自 GDI ui 迁入，单一事实来源）。

/// 翻译语言表（与前端 `translation-languages` 对齐）。
pub const LANG_TABLE: &[(&str, &str)] = &[
    ("auto", "自动检测"),
    ("zh-CN", "简体中文"),
    ("zh-TW", "繁體中文"),
    ("en", "English"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("pt", "Português"),
    ("ru", "Русский"),
    ("it", "Italiano"),
    ("nl", "Nederlands"),
    ("pl", "Polski"),
    ("tr", "Türkçe"),
    ("ar", "العربية"),
    ("th", "ไทย"),
    ("vi", "Tiếng Việt"),
    ("id", "Bahasa Indonesia"),
    ("hi", "हिन्दी"),
];

/// 语言 code → 显示名。
pub fn lang_display_name(code: &str) -> &str {
    let c = code.trim();
    LANG_TABLE
        .iter()
        .find(|(k, _)| *k == c)
        .map(|(_, n)| *n)
        .unwrap_or(if c.is_empty() { "—" } else { c })
}

/// 某侧可选语言 code 列表。
pub fn lang_codes_for_side(is_source: bool) -> Vec<&'static str> {
    LANG_TABLE
        .iter()
        .filter(|(code, _)| is_source || *code != "auto")
        .map(|(code, _)| *code)
        .collect()
}

/// 交换语言：auto 规则对齐原型（源/目标为 auto 时落到 en）。
///
/// 例：`("auto","zh-CN") -> ("zh-CN","en")`；`("en","zh-CN") -> ("zh-CN","en")`。
pub fn swap_session_langs(source: &str, target: &str) -> (String, String) {
    let new_source = if target == "auto" {
        "en".to_string()
    } else {
        target.to_string()
    };
    let new_target = if source == "auto" {
        "en".to_string()
    } else {
        source.to_string()
    };
    (new_source, new_target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_auto_keeps_auto_on_source() {
        // 与现网 GDI 一致：源 auto 交换后目标落到 en，源取原目标
        assert_eq!(
            swap_session_langs("auto", "zh-CN"),
            ("zh-CN".into(), "en".into())
        );
    }

    #[test]
    fn swap_session_langs_plain_pair() {
        assert_eq!(
            swap_session_langs("en", "zh-CN"),
            ("zh-CN".into(), "en".into())
        );
    }

    #[test]
    fn swap_exchanges_concrete_langs() {
        let (s, t) = swap_session_langs("en", "zh-CN");
        assert_eq!(s, "zh-CN");
        assert_eq!(t, "en");
    }

    #[test]
    fn lang_display_zh_cn() {
        assert_eq!(lang_display_name("zh-CN"), "简体中文");
    }

    #[test]
    fn lang_display_and_codes() {
        assert_eq!(lang_display_name("auto"), "自动检测");
        let src = lang_codes_for_side(true);
        let tgt = lang_codes_for_side(false);
        assert!(src.contains(&"auto"));
        assert!(!tgt.contains(&"auto"));
        assert!(tgt.contains(&"en"));
    }
}

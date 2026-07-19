/// source=auto 时的输出清洗状态机，供 LLM provider 复用。
/// 非 auto 场景 provider 不应使用此 parser（直通 Delta）。
///
/// 按行持续处理（不仅处理「头部」）：
/// - 识别并吞掉 `【源语言：xxx】` 标记行（含行内嵌套）
/// - 丢弃说明复读行、元标签行、原文回显行
/// - 其余行作为译文输出
///
/// 这样即使模型在译文后又复读说明，也不会把垃圾拼进卡片。
pub struct AutoLangHeaderParser {
    pending: String,
    detected: Option<String>,
    source_text: Option<String>,
    /// 是否已输出过至少一行译文。
    emitted_content: bool,
    /// 上一行已输出的非空译文（用于去掉模型反复粘贴的同一行；中间空行不打断去重）。
    last_content_line: Option<String>,
    /// 是否有待输出的段落空行（遇到下一个不同正文时再落成 `\n\n`）。
    pending_blank: bool,
}

impl AutoLangHeaderParser {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
            detected: None,
            source_text: None,
            emitted_content: false,
            last_content_line: None,
            pending_blank: false,
        }
    }

    /// 带原文：用于丢弃回显行、避免假译文=原文。
    pub fn with_source(source: impl Into<String>) -> Self {
        let source = source.into();
        let mut parser = Self::new();
        if !source.trim().is_empty() {
            parser.source_text = Some(source);
        }
        parser
    }

    /// 喂入一段 delta，返回本次可输出的纯译文片段。
    pub fn feed(&mut self, delta: &str) -> Vec<String> {
        self.pending.push_str(delta);
        self.drain_lines(false)
    }

    /// 流结束：冲刷最后一行；返回检测到的语言。
    pub fn finish(&mut self) -> (Vec<String>, Option<String>) {
        let pieces = self.drain_lines(true);
        (pieces, self.detected.clone())
    }

    fn is_source_echo(&self, line: &str) -> bool {
        let Some(src) = self.source_text.as_deref() else {
            return false;
        };
        line.trim() == src.trim()
    }

    fn drain_lines(&mut self, finalize: bool) -> Vec<String> {
        let mut out = Vec::new();

        loop {
            let Some(pos) = self.pending.find('\n') else {
                break;
            };
            let line = self.pending[..pos].to_string();
            self.pending = self.pending[pos + 1..].to_string();
            if let Some(piece) = self.take_content_line(&line) {
                out.push(piece);
            }
        }

        if finalize && !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            if let Some(piece) = self.take_content_line(&line) {
                out.push(piece);
            }
        }

        out
    }

    /// 过滤后输出一行译文；自动补换行、去掉重复粘贴的同一正文行。
    fn take_content_line(&mut self, line: &str) -> Option<String> {
        let content = self.handle_line(line)?;

        // 空行：推迟到下一段正文前再输出，避免「你好\n\n你好」去重失败
        if content.is_empty() {
            if self.emitted_content {
                self.pending_blank = true;
            }
            return None;
        }

        // 与上一非空译文相同：丢弃（智谱反复粘贴同一句）
        if self
            .last_content_line
            .as_deref()
            .is_some_and(|prev| prev == content)
        {
            self.pending_blank = false;
            return None;
        }

        let piece = if !self.emitted_content {
            content.clone()
        } else if self.pending_blank {
            format!("\n\n{content}")
        } else {
            format!("\n{content}")
        };
        self.emitted_content = true;
        self.pending_blank = false;
        self.last_content_line = Some(content);
        Some(piece)
    }

    /// 处理单行：返回应展示的译文文本（不含前导换行）。空行返回 `Some("")`。
    fn handle_line(&mut self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Some(String::new());
        }

        // 先尝试抽标记（即使整行是说明复读，也可能嵌着【源语言：英语】）
        if let Some((lang, rest)) = extract_marker(trimmed) {
            if is_valid_detected_lang(&lang) && self.detected.is_none() {
                self.detected = Some(lang);
            }
            // 整行是说明复读：只记语言，不输出
            if is_instruction_echo(trimmed) || is_meta_label(trimmed) {
                return None;
            }
            let rest = rest.trim();
            if rest.is_empty() {
                return None;
            }
            if is_instruction_echo(rest) || is_meta_label(rest) || self.is_source_echo(rest) {
                return None;
            }
            return Some(rest.to_string());
        }

        // 说明复读 / 元标签（无嵌套标记）
        if is_instruction_echo(trimmed) || is_meta_label(trimmed) {
            return None;
        }

        // 原文回显
        if self.is_source_echo(trimmed) {
            return None;
        }

        Some(trimmed.to_string())
    }
}

impl Default for AutoLangHeaderParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 模型复读 auto 检测说明的行。
fn is_instruction_echo(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    const PATTERNS: &[&str] = &[
        "先在第一行",
        "请先单独一行输出",
        "请先单独一行",
        "然后换行输出完整译文",
        "换行后再输出完整译文",
        "不要复述原文或本说明",
        "不要复述原文",
        "不要复述本说明",
        "禁止复述本说明",
        "【输出格式",
        "【任务】",
        "输出时先写一行【源语言",
    ];
    PATTERNS.iter().any(|p| t.contains(p))
}

/// 「以下为完整译文：」一类引导标签，不是译文正文。
fn is_meta_label(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    const PATTERNS: &[&str] = &[
        "以下为完整译文",
        "以下是完整译文",
        "完整译文如下",
        "译文如下",
        "翻译结果如下",
        "翻译如下",
    ];
    if PATTERNS.iter().any(|p| t.contains(p)) {
        return true;
    }
    // 整行过短且以冒号结尾的引导语
    if t.chars().count() <= 16 && (t.ends_with('：') || t.ends_with(':')) {
        if t.contains("译文") || t.contains("翻译") || t.contains("结果") {
            return true;
        }
    }
    false
}

fn is_valid_detected_lang(name: &str) -> bool {
    let n = name.trim();
    if n.is_empty() {
        return false;
    }
    let lower = n.to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "auto detect"
            | "auto-detect"
            | "autodetect"
            | "auto"
            | "自动检测"
            | "语言名称"
            | "语言名"
            | "源语言"
            | "原文"
            | "原文语言"
            | "待检测的源语言"
            | "xx"
            | "unknown"
            | "n/a"
            | "none"
    )
}

/// 从一行中提取源语言标记。成功返回 `(语言名, 同行标记后的剩余文本)`。
fn extract_marker(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(result) = extract_marker_at(trimmed) {
        return Some(result);
    }

    if let Some(idx) = trimmed.find("【源语言") {
        return extract_marker_at(trimmed[idx..].trim());
    }
    if let Some(idx) = trimmed.find("[源语言") {
        return extract_marker_at(trimmed[idx..].trim());
    }

    None
}

fn extract_marker_at(stripped_input: &str) -> Option<(String, String)> {
    let stripped = stripped_input.trim_matches(|c: char| matches!(c, '*' | '`' | '"' | '\''));

    const KEY: &str = "源语言";
    let key_pos = stripped.find(KEY)?;

    let before = stripped[..key_pos].trim();
    if !before
        .chars()
        .all(|c| matches!(c, '【' | '[' | '*' | ' ' | '\t'))
    {
        return None;
    }

    let after_key = stripped[key_pos + KEY.len()..].trim_start();
    let after_colon = after_key
        .strip_prefix('：')
        .or_else(|| after_key.strip_prefix(':'))?
        .trim_start();

    if after_colon.is_empty() {
        return None;
    }

    if let Some(end) = after_colon.find('】') {
        let name = after_colon[..end].trim();
        if name.is_empty() {
            return None;
        }
        let after = after_colon[end + '】'.len_utf8()..].trim_start();
        let after = after
            .trim_start_matches(|c: char| matches!(c, '*' | '`' | '"' | '\'' | ']'))
            .trim_start();
        // 去掉标记后常见的全角/半角逗号再接说明
        let after = after
            .trim_start_matches(|c: char| matches!(c, '，' | ',' | '；' | ';' | ' ' | '\t'))
            .trim_start();
        return Some((name.to_string(), after.to_string()));
    }

    if let Some(end) = after_colon.find(']') {
        let name = after_colon[..end].trim();
        if name.is_empty() {
            return None;
        }
        let after = after_colon[end + 1..].trim_start();
        return Some((name.to_string(), after.to_string()));
    }

    let name = after_colon.trim();
    if name.is_empty() || name.chars().count() > 32 {
        return None;
    }
    if name.split_whitespace().count() > 3 {
        return None;
    }
    Some((name.to_string(), String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(p: &mut AutoLangHeaderParser, input: &str) -> (String, Option<String>) {
        let mut pieces = p.feed(input);
        let (more, detected) = p.finish();
        pieces.extend(more);
        // pieces 内已含行间 \n（第二行起带前导换行）
        let text = pieces.concat();
        (text, detected)
    }

    #[test]
    fn feed_parses_marker_and_returns_translation_after_newline() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "【源语言：英语】\n译文内容");
        assert_eq!(text, "译文内容");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_passes_through_when_no_marker() {
        let mut p = AutoLangHeaderParser::new();
        let pieces = p.feed("译文无标记");
        assert!(pieces.is_empty(), "无 \\n 时 feed 不输出");
        let (pieces, detected) = p.finish();
        assert_eq!(pieces, vec!["译文无标记".to_string()]);
        assert_eq!(detected, None);
    }

    #[test]
    fn feed_handles_marker_split_across_chunks() {
        let mut p = AutoLangHeaderParser::new();
        assert!(p.feed("【源语言：英").is_empty());
        // 无尾部 \n 时正文留在 pending，finish 再冲刷
        assert!(p.feed("语】\n译文").is_empty());
        let (pieces, detected) = p.finish();
        assert_eq!(pieces, vec!["译文".to_string()]);
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_passes_through_first_line_when_newline_but_no_marker() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "译文第一行\n译文第二行");
        assert_eq!(text, "译文第一行\n译文第二行");
        assert_eq!(detected, None);
    }

    #[test]
    fn feed_passes_through_after_marker() {
        let mut p = AutoLangHeaderParser::new();
        p.feed("【源语言：英语】\n");
        let pieces = p.feed("后续译文");
        // 无换行，等 finish
        assert!(pieces.is_empty());
        let (more, _) = p.finish();
        assert_eq!(more, vec!["后续译文".to_string()]);
    }

    #[test]
    fn feed_parses_halfwidth_colon_marker() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "【源语言:英语】\n你好");
        assert_eq!(text, "你好");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_parses_marker_without_brackets() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "源语言：英语\n你好");
        assert_eq!(text, "你好");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_parses_marker_and_translation_on_same_line() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "【源语言：英语】你好");
        assert_eq!(text, "你好");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_skips_leading_blank_lines_before_marker() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "\n\n【源语言：英语】\n你好");
        assert_eq!(text, "你好");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_parses_markdown_wrapped_marker() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "**【源语言：英语】**\n你好");
        assert_eq!(text, "你好");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_strips_source_echo_then_translation() {
        let mut p = AutoLangHeaderParser::with_source("hello");
        let (text, detected) = feed_all(&mut p, "hello\n您好");
        assert_eq!(text, "您好");
        assert_eq!(detected, None);
    }

    #[test]
    fn finish_marker_only_detects_without_emitting_marker() {
        let mut p = AutoLangHeaderParser::new();
        assert!(p.feed("【源语言：英语】").is_empty());
        let (pieces, detected) = p.finish();
        assert!(pieces.is_empty(), "仅标记时不应输出标记本身: {pieces:?}");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_trims_blank_line_after_marker() {
        let mut p = AutoLangHeaderParser::new();
        let (text, _) = feed_all(&mut p, "【源语言：英语】\n\n你好");
        assert_eq!(text, "你好");
    }

    #[test]
    fn strips_zhipu_repeated_instruction_and_keeps_translation() {
        // 用户实测：先出译文，再反复复读说明 + 原文 + 译文
        let mut p = AutoLangHeaderParser::with_source("hello");
        let input = "\
你好

请先单独一行输出【源语言：语言名】（如：英语），换行后再输出完整译文；不要复述原文或本说明。

请先单独一行输出【源语言：英语】，换行后再输出完整译文；不要复述原文或本说明。

以下为完整译文：

hello

你好

请先单独一行输出【源语言：英语】，换行后再输出完整译文；不要复述原文或本说明。

请先单独一行输出【源语言：英语】，换行后再输出完整译文；不要复述原文或本说明。

以下为完整译文：

hello

你好
";
        let (text, detected) = feed_all(&mut p, input);
        // 只保留译文行；重复的「你好」可保留多次或去重——至少不得含说明/原文
        assert!(!text.contains("请先单独一行"), "不得含说明复读: {text}");
        assert!(!text.contains("以下为完整译文"), "不得含元标签: {text}");
        assert!(!text.contains("hello"), "不得含原文回显: {text}");
        assert_eq!(text, "你好", "清洗后应只剩一行译文: {text:?}");
        // 检测语言来自复读行里的嵌套标记
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn rejects_auto_detect_as_detected_language_then_parses_real_marker() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "【源语言：Auto Detect】\n【源语言：英语】\n你好");
        assert_eq!(text, "你好");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn extract_marker_from_prefixed_instruction_line() {
        let mut p = AutoLangHeaderParser::new();
        let (text, detected) = feed_all(&mut p, "先输出：【源语言：日语】\nこんにちは");
        assert_eq!(text, "こんにちは");
        assert_eq!(detected, Some("日语".to_string()));
    }

    #[test]
    fn marker_then_source_only_body_emits_nothing() {
        let mut p = AutoLangHeaderParser::with_source("hello");
        let (text, detected) = feed_all(&mut p, "【源语言：英语】\nhello");
        assert_eq!(text, "");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn pure_source_echo_without_translation_emits_nothing() {
        let mut p = AutoLangHeaderParser::with_source("yes");
        let (text, detected) = feed_all(&mut p, "yes");
        assert_eq!(text, "");
        assert_eq!(detected, None);
    }

    #[test]
    fn marker_source_echo_then_real_translation() {
        let mut p = AutoLangHeaderParser::with_source("yes");
        let (text, detected) = feed_all(&mut p, "【源语言：英语】\nyes\n是的");
        assert_eq!(text, "是的");
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn multi_line_translation_preserved() {
        let mut p = AutoLangHeaderParser::with_source("hi");
        let (text, _) = feed_all(&mut p, "【源语言：英语】\n第一段\n\n第二段");
        assert_eq!(text, "第一段\n\n第二段");
    }
}

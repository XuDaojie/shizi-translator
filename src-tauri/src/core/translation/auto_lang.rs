/// source=auto 时的首行 `【源语言：xxx】` 解析状态机，供 LLM provider 复用。
/// 非 auto 场景 provider 不应使用此 parser（直通 Delta）。
pub struct AutoLangHeaderParser {
    pending: String,
    parsed: bool,
    detected: Option<String>,
}

impl AutoLangHeaderParser {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
            parsed: false,
            detected: None,
        }
    }

    /// 喂入一段 delta，返回本次可输出的纯译文片段（标记行被吞掉；标记不匹配则首行作 Delta 补发）。
    pub fn feed(&mut self, delta: &str) -> Vec<String> {
        if self.parsed {
            return vec![delta.to_string()];
        }
        self.pending.push_str(delta);
        let Some(pos) = self.pending.find('\n') else {
            return Vec::new();
        };
        let first_line = self.pending[..pos].to_string();
        let rest = self.pending[pos + 1..].to_string();
        self.parsed = true;
        self.detected = parse_detected_lang(&first_line);
        self.pending.clear();
        let mut out = Vec::new();
        if self.detected.is_none() {
            out.push(first_line);
        }
        if !rest.is_empty() {
            out.push(rest);
        }
        out
    }

    /// 流结束后：若首行未解析且 pending 非空，作为译文补出；返回检测到的语言。
    pub fn finish(&mut self) -> (Vec<String>, Option<String>) {
        let mut out = Vec::new();
        if !self.parsed && !self.pending.is_empty() {
            out.push(std::mem::take(&mut self.pending));
        }
        (out, self.detected.clone())
    }
}

impl Default for AutoLangHeaderParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 从首行 `【源语言：xxx】` 提取语言名；不匹配返回 None。
fn parse_detected_lang(first_line: &str) -> Option<String> {
    const PREFIX: &str = "【源语言：";
    let start = first_line.find(PREFIX)?;
    let after = &first_line[start + PREFIX.len()..];
    let end = after.find('】')?;
    let name = after[..end].trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_parses_marker_and_returns_translation_after_newline() {
        let mut p = AutoLangHeaderParser::new();
        let pieces = p.feed("【源语言：英语】\n译文内容");
        assert_eq!(pieces, vec!["译文内容".to_string()]);
        let (_, detected) = p.finish();
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
        let pieces = p.feed("语】\n译文");
        assert_eq!(pieces, vec!["译文".to_string()]);
        let (_, detected) = p.finish();
        assert_eq!(detected, Some("英语".to_string()));
    }

    #[test]
    fn feed_passes_through_first_line_when_newline_but_no_marker() {
        let mut p = AutoLangHeaderParser::new();
        let pieces = p.feed("译文第一行\n译文第二行");
        // 无标记但含 \n：首行补发 + 后续行
        assert_eq!(pieces, vec!["译文第一行".to_string(), "译文第二行".to_string()]);
        let (_, detected) = p.finish();
        assert_eq!(detected, None);
    }

    #[test]
    fn feed_passes_through_after_parsed() {
        let mut p = AutoLangHeaderParser::new();
        p.feed("【源语言：英语】\n");
        let pieces = p.feed("后续译文");
        assert_eq!(pieces, vec!["后续译文".to_string()]);
    }
}

use std::{sync::Arc, sync::Mutex};

use crate::core::llm::{LlmError, LlmProvider, LlmStreamEvent};
use tokio_util::sync::CancellationToken;

use super::{
    TokenUsage, TranslationEvent, TranslationRequest,
};

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error(transparent)]
    Llm(#[from] LlmError),
}

impl TranslationError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::Llm(error) => error.retryable(),
        }
    }
}

#[derive(Clone)]
pub struct TranslationService {
    provider: Arc<dyn LlmProvider>,
}

impl TranslationService {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub async fn translate_with<F>(
        &self,
        request: TranslationRequest,
        collect_usage: bool,
        cancel: CancellationToken,
        mut emit: F,
    ) -> Result<(), TranslationError>
    where
        F: FnMut(TranslationEvent) + Send,
    {
        log::info!(
            "翻译开始: service={} protocol={} session={}",
            request.service.service_name,
            request.service.protocol,
            request.session_id.0
        );

        let is_auto = request.prompts.source_lang == "auto";
        let full_text = Arc::new(Mutex::new(String::new()));
        let usage: Arc<Mutex<Option<TokenUsage>>> = Arc::new(Mutex::new(None));
        let header_state = Arc::new(Mutex::new(HeaderParseState {
            pending: String::new(),
            parsed: false,
            detected: None,
        }));
        let delta_text = full_text.clone();
        let usage_slot = usage.clone();
        let header_slot = header_state.clone();
        let delta_session_id = request.session_id.clone();
        let delta_service = request.service.clone();

        self.provider
            .stream_translate(&request, &mut |ev| {
                match ev {
                    LlmStreamEvent::Delta(text) => {
                        let pieces = if is_auto {
                            process_auto_delta(&header_slot, &text)
                        } else {
                            vec![text]
                        };
                        for piece in pieces {
                            if let Ok(mut t) = delta_text.lock() {
                                t.push_str(&piece);
                            }
                            emit(TranslationEvent::Delta {
                                session_id: delta_session_id.clone(),
                                service: delta_service.clone(),
                                text: piece,
                            });
                        }
                    }
                    LlmStreamEvent::Usage(u) => {
                        if collect_usage {
                            if let Ok(mut slot) = usage_slot.lock() {
                                *slot = Some(u);
                            }
                        }
                    }
                }
            }, &cancel)
            .await?;

        // 译文极短无 `\n`：首行未解析，pending 累积的内容补作 Delta（不丢译文），detected 为 None。
        let detected = if is_auto {
            let mut st = header_state.lock().unwrap();
            if !st.parsed && !st.pending.is_empty() {
                let pending = std::mem::take(&mut st.pending);
                if let Ok(mut t) = delta_text.lock() {
                    t.push_str(&pending);
                }
                emit(TranslationEvent::Delta {
                    session_id: delta_session_id.clone(),
                    service: delta_service.clone(),
                    text: pending,
                });
            }
            st.detected.clone()
        } else {
            None
        };

        let full_text = full_text
            .lock()
            .map(|text| text.clone())
            .unwrap_or_default();

        if cancel.is_cancelled() {
            log::warn!(
                "翻译取消: service={} session={}",
                request.service.service_name,
                request.session_id.0
            );
            emit(TranslationEvent::Cancelled {
                session_id: request.session_id,
                service: request.service,
            });
        } else {
            let usage = usage
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or(None);
            log::info!(
                "翻译完成: service={} session={} len={}",
                request.service.service_name,
                request.session_id.0,
                full_text.chars().count()
            );
            emit(TranslationEvent::Finished {
                session_id: request.session_id,
                service: request.service,
                full_text,
                usage,
                detected_source_lang: detected,
            });
        }

        Ok(())
    }
}

/// source=auto 时的首行解析状态。
struct HeaderParseState {
    /// 累积首行字符，直到遇到首个 `\n`。
    pending: String,
    /// 首行是否已解析完毕。
    parsed: bool,
    /// 解析到的语言名（匹配 `【源语言：xxx】`）；未匹配为 None。
    detected: Option<String>,
}

/// 从首行 `【源语言：xxx】` 提取语言名；不匹配返回 None。
/// 用字符串查找而非正则，避免引入 regex 依赖。
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

/// source=auto 时的流式首行解析：累积到首个 `\n` 后解析标记行，
/// 返回应作为 Delta 发出的纯译文片段（标记行被吞掉；标记不匹配则首行作 Delta 补发，不吞译文）。
fn process_auto_delta(state: &Mutex<HeaderParseState>, text: &str) -> Vec<String> {
    let mut st = state.lock().unwrap();
    if st.parsed {
        return vec![text.to_string()];
    }
    st.pending.push_str(text);
    let Some(pos) = st.pending.find('\n') else {
        return Vec::new();
    };
    let first_line = st.pending[..pos].to_string();
    let rest = st.pending[pos + 1..].to_string();
    st.parsed = true;
    st.detected = parse_detected_lang(&first_line);
    st.pending.clear();
    let mut out = Vec::new();
    if st.detected.is_none() {
        out.push(first_line);
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm::{LlmProvider, LlmStreamEvent};
    use crate::core::translation::{
        TokenUsage, TranslationInput, TranslationPromptConfig, TranslationRequest,
        TranslationServiceMeta, TranslationSessionId,
    };
    use std::sync::{Arc, Mutex};
    use tokio_util::sync::CancellationToken;

    struct CancelAwareFakeProvider {
        deltas_emitted: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for CancelAwareFakeProvider {
        async fn stream_translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
            cancel: &CancellationToken,
        ) -> Result<(), LlmError> {
            let chunks = ["a", "b", "c"];
            for chunk in chunks {
                tokio::select! {
                    _ = cancel.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
                }
                on_event(LlmStreamEvent::Delta(chunk.to_string()));
                self.deltas_emitted.lock().unwrap().push(chunk.to_string());
            }
            Ok(())
        }
    }

    struct UsageFakeProvider;

    #[async_trait::async_trait]
    impl LlmProvider for UsageFakeProvider {
        async fn stream_translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
            _cancel: &CancellationToken,
        ) -> Result<(), LlmError> {
            on_event(LlmStreamEvent::Delta("你好".to_string()));
            on_event(LlmStreamEvent::Usage(TokenUsage {
                input_tokens: 27,
                output_tokens: 18,
            }));
            Ok(())
        }
    }

    fn fake_service() -> TranslationServiceMeta {
        TranslationServiceMeta {
            service_instance_id: "test".to_string(),
            service_name: "test".to_string(),
            service_type: "llm".to_string(),
            protocol: "mock".to_string(),
        }
    }

    fn request() -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test-session".to_string()),
            input: TranslationInput::ManualText("hi".to_string()),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: Default::default(),
        }
    }

    #[tokio::test]
    async fn emits_cancelled_when_cancelled_before_completion() {
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let provider = CancelAwareFakeProvider {
            deltas_emitted: emitted.clone(),
        };
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        let handle = tokio::spawn(async move {
            service
                .translate_with(request(), true, cancel_for_task, |event| {
                    events_for_task.lock().unwrap().push(event);
                })
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        cancel.cancel();

        handle.await.expect("task 未 panic").expect("应返回 Ok");

        let events = events.lock().unwrap();
        let types: Vec<&str> = events.iter().map(|e| match e {
            TranslationEvent::Started { .. } => "started",
            TranslationEvent::Delta { .. } => "delta",
            TranslationEvent::Finished { .. } => "finished",
            TranslationEvent::Failed { .. } => "failed",
            TranslationEvent::Cancelled { .. } => "cancelled",
        }).collect();

        assert!(types.contains(&"cancelled"), "应 emit Cancelled: {:?}", types);
        assert!(!types.contains(&"finished"), "取消时不应 emit Finished");
    }

    #[tokio::test]
    async fn emits_finished_when_not_cancelled() {
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let provider = CancelAwareFakeProvider {
            deltas_emitted: emitted.clone(),
        };
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(request(), true, cancel, |event| {
                events_for_task.lock().unwrap().push(event);
            })
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let types: Vec<&str> = events.iter().map(|e| match e {
            TranslationEvent::Started { .. } => "started",
            TranslationEvent::Delta { .. } => "delta",
            TranslationEvent::Finished { .. } => "finished",
            TranslationEvent::Failed { .. } => "failed",
            TranslationEvent::Cancelled { .. } => "cancelled",
        }).collect();

        assert!(types.contains(&"finished"), "未取消应 emit Finished: {:?}", types);
        assert!(!types.contains(&"cancelled"), "未取消不应 emit Cancelled");
    }

    #[tokio::test]
    async fn finished_carries_usage_when_collect_enabled() {
        let service = TranslationService::new(Arc::new(UsageFakeProvider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(
                request(),
                true,
                cancel,
                |event| events_for_task.lock().unwrap().push(event),
            )
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let usage = events.iter().find_map(|e| match e {
            TranslationEvent::Finished { usage, .. } => usage.clone(),
            _ => None,
        });
        assert_eq!(
            usage,
            Some(TokenUsage {
                input_tokens: 27,
                output_tokens: 18
            })
        );
    }

    #[tokio::test]
    async fn finished_usage_none_when_collect_disabled() {
        let service = TranslationService::new(Arc::new(UsageFakeProvider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();

        service
            .translate_with(
                request(),
                false,
                cancel,
                |event| events_for_task.lock().unwrap().push(event),
            )
            .await
            .expect("应返回 Ok");

        let events = events.lock().unwrap();
        let usage = events.iter().find_map(|e| match e {
            TranslationEvent::Finished { usage, .. } => usage.clone(),
            _ => None,
        });
        assert_eq!(usage, None);
    }

    /// 可按预设 chunks 输出 Delta 的 fake provider，用于状态机测试。
    struct DetectFakeProvider {
        chunks: Vec<String>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for DetectFakeProvider {
        async fn stream_translate(
            &self,
            _request: &TranslationRequest,
            on_event: &mut (dyn FnMut(LlmStreamEvent) + Send),
            _cancel: &CancellationToken,
        ) -> Result<(), LlmError> {
            for chunk in &self.chunks {
                on_event(LlmStreamEvent::Delta(chunk.clone()));
            }
            Ok(())
        }
    }

    fn request_with_source(source_lang: &str) -> TranslationRequest {
        TranslationRequest {
            session_id: TranslationSessionId("test-session".to_string()),
            input: TranslationInput::ManualText("hello".to_string()),
            target_lang: "中文".to_string(),
            service: fake_service(),
            prompts: TranslationPromptConfig {
                source_lang: source_lang.to_string(),
                ..Default::default()
            },
        }
    }

    fn collect_deltas(events: &[TranslationEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                TranslationEvent::Delta { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn collect_detected(events: &[TranslationEvent]) -> Option<String> {
        events.iter().find_map(|e| match e {
            TranslationEvent::Finished {
                detected_source_lang, ..
            } => detected_source_lang.clone(),
            _ => None,
        })
    }

    async fn run_translate(provider: DetectFakeProvider, source_lang: &str) -> Vec<TranslationEvent> {
        let service = TranslationService::new(Arc::new(provider));
        let cancel = CancellationToken::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_task = events.clone();
        service
            .translate_with(request_with_source(source_lang), true, cancel, |event| {
                events_for_task.lock().unwrap().push(event);
            })
            .await
            .expect("应返回 Ok");
        let events = events.lock().unwrap();
        events.clone()
    }

    #[tokio::test]
    async fn translate_detects_source_lang_from_header() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["【源语言：英语】\n译文内容".to_string()],
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文内容");
        assert_eq!(collect_detected(&events), Some("英语".to_string()));
    }

    #[tokio::test]
    async fn translate_fallbacks_when_no_header_marker() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["译文无标记".to_string()],
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文无标记");
        assert_eq!(collect_detected(&events), None);
    }

    #[tokio::test]
    async fn translate_passes_through_when_no_marker_but_has_newline() {
        // 无标记但含 \n：走 process_auto_delta 内 detected.is_none() 首行补发分支
        // （与 translate_fallbacks_when_no_header_marker 的「无 \n 走 stream 后 pending 补发」区分），
        // 验证不吞译文：首行与后续行均作 Delta 透传，\n 被切分吞掉，detected 为 None。
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["译文第一行\n译文第二行".to_string()],
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文第一行译文第二行");
        assert_eq!(collect_detected(&events), None);
    }

    #[tokio::test]
    async fn translate_handles_marker_across_chunks() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["【源语言：英".to_string(), "语】\n译文".to_string()],
            },
            "auto",
        )
        .await;
        assert_eq!(collect_deltas(&events), "译文");
        assert_eq!(collect_detected(&events), Some("英语".to_string()));
    }

    #[tokio::test]
    async fn translate_does_not_parse_when_source_specific() {
        let events = run_translate(
            DetectFakeProvider {
                chunks: vec!["【源语言：英语】\n译文".to_string()],
            },
            "en-US",
        )
        .await;
        assert_eq!(collect_deltas(&events), "【源语言：英语】\n译文");
        assert_eq!(collect_detected(&events), None);
    }
}


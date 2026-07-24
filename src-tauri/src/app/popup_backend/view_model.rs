//! PopupViewModel 纯函数：将 TranslationEvent 归并进 ViewModel。
//! 语义对齐前端 `useTranslationEvents`（batch 前缀 + 按 service_instance_id 卡片）。

use crate::core::translation::{TokenUsage, TranslationEvent, TranslationServiceMeta};

use super::types::{PopupCardStatus, PopupCardVm, PopupViewModel};

/// 将一条翻译事件归并进 ViewModel（无 I/O、无副作用）。
pub fn apply_translation_event(vm: &mut PopupViewModel, event: &TranslationEvent) {
    match event {
        TranslationEvent::Started {
            session_id,
            service,
            source_text,
            source_type,
        } => {
            let sid = session_id.0.as_str();
            let batch = batch_id_from_session(sid);
            let is_new_batch = match vm.session_id.as_deref() {
                Some(current) => batch_id_from_session(current) != batch,
                None => true,
            };

            if is_new_batch {
                // 新批次：重置已有卡片文本状态，开启 translating
                for card in &mut vm.cards {
                    reset_card_for_new_batch(card);
                }
                vm.is_translating = true;
                vm.source_text = source_text.clone();
                vm.source_type = source_type.clone();
            }

            // 记录当前 session（整段 id，batch 前缀用于后续匹配）
            vm.session_id = Some(sid.to_string());

            let card = ensure_card(vm, service);
            apply_service_meta(card, service);
            card.status = PopupCardStatus::Translating;
            card.text.clear();
            card.error_message.clear();
            card.usage_input = None;
            card.usage_output = None;
            card.detected_source_lang = None;
        }
        TranslationEvent::Delta {
            session_id,
            service,
            text,
        } => {
            if !session_matches_current_batch(vm, &session_id.0) {
                return;
            }
            if let Some(card) = find_card_mut(vm, &service.service_instance_id) {
                card.text.push_str(text);
            }
        }
        TranslationEvent::Finished {
            session_id,
            service,
            full_text,
            usage,
            detected_source_lang,
        } => {
            if !session_matches_current_batch(vm, &session_id.0) {
                return;
            }
            if let Some(card) = find_card_mut(vm, &service.service_instance_id) {
                card.text = full_text.clone();
                card.status = PopupCardStatus::Finished;
                let (input, output) = map_usage(usage.as_ref());
                card.usage_input = input;
                card.usage_output = output;
                card.detected_source_lang = detected_source_lang.clone();
                card.error_message.clear();
            }
            refresh_is_translating(vm);
        }
        TranslationEvent::Failed {
            session_id,
            service,
            message,
            retryable: _,
        } => {
            if !session_matches_current_batch(vm, &session_id.0) {
                return;
            }
            if let Some(card) = find_card_mut(vm, &service.service_instance_id) {
                card.status = PopupCardStatus::Failed;
                card.error_message = message.clone();
                card.usage_input = None;
                card.usage_output = None;
            }
            refresh_is_translating(vm);
        }
        TranslationEvent::Cancelled {
            session_id,
            service,
        } => {
            if !session_matches_current_batch(vm, &session_id.0) {
                return;
            }
            if let Some(card) = find_card_mut(vm, &service.service_instance_id) {
                card.status = PopupCardStatus::Cancelled;
            }
            refresh_is_translating(vm);
        }
    }
}

/// 从 `batchId:serviceInstanceId` 形式的 session_id 提取 batch 前缀。
fn batch_id_from_session(session_id: &str) -> &str {
    session_id
        .split_once(':')
        .map(|(b, _)| b)
        .unwrap_or(session_id)
}

/// 非 Started 事件：当前 ViewModel 有 session，且 batch 前缀一致才处理。
/// （等价于「同批次」；同批次多服务用 service_instance_id 定位卡片。）
fn session_matches_current_batch(vm: &PopupViewModel, event_session_id: &str) -> bool {
    match vm.session_id.as_deref() {
        Some(current) => {
            batch_id_from_session(current) == batch_id_from_session(event_session_id)
        }
        None => false,
    }
}

fn ensure_card<'a>(vm: &'a mut PopupViewModel, service: &TranslationServiceMeta) -> &'a mut PopupCardVm {
    let id = &service.service_instance_id;
    if let Some(pos) = vm
        .cards
        .iter()
        .position(|c| c.service_instance_id == *id)
    {
        return &mut vm.cards[pos];
    }
    vm.cards.push(PopupCardVm {
        service_instance_id: service.service_instance_id.clone(),
        service_name: service.service_name.clone(),
        service_type: service.service_type.clone(),
        protocol: service.protocol.clone(),
        model_name: service.model_name.clone(),
        status: PopupCardStatus::Pending,
        text: String::new(),
        error_message: String::new(),
        usage_input: None,
        usage_output: None,
        detected_source_lang: None,
    });
    vm.cards.last_mut().expect("just pushed")
}

fn find_card_mut<'a>(
    vm: &'a mut PopupViewModel,
    service_instance_id: &str,
) -> Option<&'a mut PopupCardVm> {
    vm.cards
        .iter_mut()
        .find(|c| c.service_instance_id == service_instance_id)
}

fn apply_service_meta(card: &mut PopupCardVm, service: &TranslationServiceMeta) {
    card.service_name = service.service_name.clone();
    card.service_type = service.service_type.clone();
    card.protocol = service.protocol.clone();
    card.model_name = service.model_name.clone();
}

fn reset_card_for_new_batch(card: &mut PopupCardVm) {
    card.status = PopupCardStatus::Pending;
    card.text.clear();
    card.error_message.clear();
    card.usage_input = None;
    card.usage_output = None;
    card.detected_source_lang = None;
}

fn map_usage(usage: Option<&TokenUsage>) -> (Option<u32>, Option<u32>) {
    match usage {
        Some(u) => (
            u32::try_from(u.input_tokens).ok(),
            u32::try_from(u.output_tokens).ok(),
        ),
        None => (None, None),
    }
}

/// 任一终态后：若无 Translating/Pending 则 is_translating=false。
fn refresh_is_translating(vm: &mut PopupViewModel) {
    let any_active = vm.cards.iter().any(|c| {
        matches!(
            c.status,
            PopupCardStatus::Translating | PopupCardStatus::Pending
        )
    });
    if !any_active {
        vm.is_translating = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::translation::{
        TokenUsage, TranslationEvent, TranslationServiceMeta, TranslationSessionId,
    };

    fn meta(id: &str) -> TranslationServiceMeta {
        TranslationServiceMeta {
            service_instance_id: id.into(),
            service_name: "Mock".into(),
            service_type: "llm".into(),
            protocol: "mock".into(),
            model_name: "m".into(),
        }
    }

    #[test]
    fn started_sets_session_and_card_translating() {
        let mut vm = PopupViewModel::default();
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Started {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                source_text: "hello".into(),
                source_type: "selectedText".into(),
            },
        );
        assert_eq!(vm.source_text, "hello");
        assert!(vm.is_translating);
        assert_eq!(vm.cards.len(), 1);
        assert_eq!(vm.cards[0].status, PopupCardStatus::Translating);
        assert_eq!(vm.cards[0].text, "");
    }

    #[test]
    fn delta_appends_text() {
        let mut vm = PopupViewModel::default();
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Started {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                source_text: "hi".into(),
                source_type: "manualText".into(),
            },
        );
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Delta {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                text: "你".into(),
            },
        );
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Delta {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                text: "好".into(),
            },
        );
        assert_eq!(vm.cards[0].text, "你好");
    }

    #[test]
    fn finished_sets_full_text_and_usage() {
        let mut vm = PopupViewModel::default();
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Started {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                source_text: "hi".into(),
                source_type: "manualText".into(),
            },
        );
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Finished {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                full_text: "你好".into(),
                usage: Some(TokenUsage {
                    input_tokens: 1,
                    output_tokens: 2,
                }),
                detected_source_lang: Some("en".into()),
            },
        );
        assert_eq!(vm.cards[0].status, PopupCardStatus::Finished);
        assert_eq!(vm.cards[0].text, "你好");
        assert_eq!(vm.cards[0].usage_input, Some(1));
        assert_eq!(vm.cards[0].detected_source_lang.as_deref(), Some("en"));
    }

    #[test]
    fn stale_session_delta_is_ignored() {
        let mut vm = PopupViewModel::default();
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Started {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                source_text: "a".into(),
                source_type: "manualText".into(),
            },
        );
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Delta {
                session_id: TranslationSessionId("old:svc".into()),
                service: meta("svc"),
                text: "丢弃".into(),
            },
        );
        assert_eq!(vm.cards[0].text, "");
    }
}

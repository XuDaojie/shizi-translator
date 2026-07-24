//! 路径 R：线程安全 ViewModel 快照与复制纯函数（无窗口）。

use std::sync::{Arc, Mutex, OnceLock};

use crate::app::popup_backend::types::PopupViewModel;

/// 供 Reactor UI 线程与 backend `publish` 共享的 ViewModel 快照。
#[derive(Clone, Default)]
pub struct SharedPopupState {
    inner: Arc<Mutex<PopupViewModel>>,
}

impl SharedPopupState {
    pub fn store(&self, vm: &PopupViewModel) {
        if let Ok(mut g) = self.inner.lock() {
            *g = vm.clone();
        }
    }

    pub fn load(&self) -> PopupViewModel {
        self.inner
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

static GLOBAL: OnceLock<SharedPopupState> = OnceLock::new();

/// 进程内全局状态桥（任务 4+ `publish` / actions 使用）。
pub fn global_state() -> &'static SharedPopupState {
    GLOBAL.get_or_init(SharedPopupState::default)
}

pub fn store_global(vm: &PopupViewModel) {
    global_state().store(vm);
}

pub fn global_snapshot() -> PopupViewModel {
    global_state().load()
}

/// 按服务实例解析可复制文案：优先译文，否则错误信息。
pub fn resolve_copy_text(vm: &PopupViewModel, service_instance_id: &str) -> Option<String> {
    let card = vm
        .cards
        .iter()
        .find(|c| c.service_instance_id == service_instance_id)?;
    resolve_copy_fields(&card.text, &card.error_message)
}

/// 字段级复制解析（PaintSnapshot / ViewModel 共用，避免双实现漂移）。
pub fn resolve_copy_fields(text: &str, error_message: &str) -> Option<String> {
    let t = text.trim();
    if !t.is_empty() {
        return Some(t.to_string());
    }
    let e = error_message.trim();
    if !e.is_empty() {
        return Some(e.to_string());
    }
    None
}

/// 第一个有非空译文的服务实例 id。
pub fn first_copyable_service_id(vm: &PopupViewModel) -> Option<String> {
    vm.cards.iter().find_map(|c| {
        if !c.text.trim().is_empty() {
            Some(c.service_instance_id.clone())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::popup_backend::types::{PopupCardStatus, PopupCardVm, PopupViewModel};

    fn card(
        id: &str,
        name: &str,
        protocol: &str,
        model: &str,
        status: PopupCardStatus,
        text: &str,
        error: &str,
        usage_input: Option<u32>,
        usage_output: Option<u32>,
    ) -> PopupCardVm {
        PopupCardVm {
            service_instance_id: id.into(),
            service_name: name.into(),
            service_type: "llm".into(),
            protocol: protocol.into(),
            model_name: model.into(),
            status,
            text: text.into(),
            error_message: error.into(),
            usage_input,
            usage_output,
            detected_source_lang: None,
        }
    }

    #[test]
    fn resolve_copy_prefers_card_text() {
        let snap = PopupViewModel {
            cards: vec![
                card(
                    "a",
                    "A",
                    "mock",
                    "m",
                    PopupCardStatus::Finished,
                    "",
                    "",
                    None,
                    None,
                ),
                card(
                    "b",
                    "B",
                    "openai_chat",
                    "gpt",
                    PopupCardStatus::Finished,
                    "你好",
                    "",
                    Some(1),
                    Some(2),
                ),
            ],
            ..Default::default()
        };
        assert_eq!(resolve_copy_text(&snap, "b").as_deref(), Some("你好"));
        assert_eq!(resolve_copy_text(&snap, "a"), None);
    }

    #[test]
    fn resolve_copy_falls_back_to_error_message() {
        let snap = PopupViewModel {
            cards: vec![card(
                "e",
                "E",
                "mock",
                "m",
                PopupCardStatus::Failed,
                "",
                "超时",
                None,
                None,
            )],
            ..Default::default()
        };
        assert_eq!(resolve_copy_text(&snap, "e").as_deref(), Some("超时"));
    }

    #[test]
    fn first_copyable_skips_empty_text() {
        let snap = PopupViewModel {
            cards: vec![
                card(
                    "a",
                    "A",
                    "mock",
                    "m",
                    PopupCardStatus::Pending,
                    "",
                    "",
                    None,
                    None,
                ),
                card(
                    "b",
                    "B",
                    "mock",
                    "m",
                    PopupCardStatus::Finished,
                    "译文",
                    "",
                    None,
                    None,
                ),
            ],
            ..Default::default()
        };
        assert_eq!(first_copyable_service_id(&snap).as_deref(), Some("b"));
    }

    #[test]
    fn shared_popup_state_store_load() {
        let st = SharedPopupState::default();
        let mut vm = PopupViewModel::default();
        vm.source_text = "hello".into();
        st.store(&vm);
        assert_eq!(st.load().source_text, "hello");
    }

    /// host 未 start / 无窗时 store 仍成功（publish pending last-write-wins）。
    #[test]
    fn publish_does_not_require_window() {
        let st = SharedPopupState::default();
        let vm = PopupViewModel {
            source_text: "hi".into(),
            ..Default::default()
        };
        st.store(&vm);
        assert_eq!(st.load().source_text, "hi");
    }

    #[test]
    fn global_snapshot_roundtrip() {
        let mut vm = PopupViewModel::default();
        vm.source_text = "global".into();
        store_global(&vm);
        assert_eq!(global_snapshot().source_text, "global");
    }
}

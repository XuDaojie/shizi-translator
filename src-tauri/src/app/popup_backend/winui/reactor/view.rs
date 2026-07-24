//! 路径 R 弹窗 UI：标题栏 + 源文 + 语言栏 + 多服务结果卡 + 取消/重试 + 状态栏。
//!
//! 状态由 host 的 `use_async_state` 驱动；本模块只渲染。
//! 动作经本模块静态 handler 分发（由 `actions::install_action_handler` 注册为
//! `handle_user_action`），避免 `view → actions → host → view` 编译期环导致
//! 测试二进制加载失败（STATUS_ENTRYPOINT_NOT_FOUND）。

#![cfg(all(windows, feature = "popup-winui"))]

use std::sync::Mutex;

use windows_reactor::{
    button, caption, hstack, scroll_viewer, text_block, vstack, Button, Color, ComboBox, Element,
    ElementExt,
};

use super::langs::{lang_codes_for_side, lang_display_name, swap_session_langs};
use super::meta::{display_model_name, should_show_tokens};
use crate::app::popup_backend::types::{
    PopupCardStatus, PopupCardVm, PopupUserAction, PopupViewModel,
};

/// 与 GDI / 原型对齐的逻辑宽度。
pub const POPUP_VIEW_WIDTH: f64 = 468.0;

/// 结果区 `scroll_viewer` 最大高度（逻辑 px）；总窗高由 host `inner_size` 约 520。
const RESULTS_SCROLL_MAX_HEIGHT: f64 = 360.0;

/// 品牌 accent：柿子橙 `#D55A1F`（系统 `.accent()` 跟 Windows 强调色，此处用资源色保品牌）。
const ACCENT_PERSIMMON: Color = Color::rgb(0xD5, 0x5A, 0x1F);
const ACCENT_ON_PERSIMMON: Color = Color::rgb(0xFF, 0xFF, 0xFF);

type UserActionHandler = fn(PopupUserAction);

static USER_ACTION_HANDLER: Mutex<Option<UserActionHandler>> = Mutex::new(None);

/// 由 `actions::install_action_handler` 注册（典型为 `handle_user_action`）。
pub fn set_user_action_handler(handler: UserActionHandler) {
    if let Ok(mut g) = USER_ACTION_HANDLER.lock() {
        *g = Some(handler);
    }
}

fn dispatch_user_action(action: PopupUserAction) {
    let handler = USER_ACTION_HANDLER.lock().ok().and_then(|g| *g);
    if let Some(h) = handler {
        h(action);
    } else {
        log::warn!("Reactor 弹窗未注册动作处理器，忽略: {action:?}");
    }
}

/// 底部状态栏文案：`is_translating` → 翻译中；源文空 → 就绪；否则完成。
fn footer_status_label(is_translating: bool, source_text: &str) -> &'static str {
    if is_translating {
        "翻译中…"
    } else if source_text.trim().is_empty() {
        "就绪"
    } else {
        "完成"
    }
}

/// 源文字数（Unicode 标量，`chars().count()`）。
fn source_char_count(source_text: &str) -> usize {
    source_text.chars().count()
}

/// 主操作按钮：柿子橙底 + 白字（品牌 accent，非系统 Accent 色）。
fn accent_button(label: impl Into<String>) -> Button {
    button(label)
        .background(ACCENT_PERSIMMON)
        .foreground(ACCENT_ON_PERSIMMON)
}

fn title_bar() -> Element {
    hstack((
        text_block("柿子翻译")
            .font_size(14.0)
            .semibold()
            .foreground(ACCENT_PERSIMMON),
        button("钉").on_click(|| {
            log::debug!("Reactor 标题栏：钉（stub）");
        }),
        button("收藏").on_click(|| {
            log::debug!("Reactor 标题栏：收藏（stub）");
        }),
        button("截图").on_click(|| {
            log::debug!("Reactor 标题栏：截图（stub）");
        }),
        button("书签").on_click(|| {
            log::debug!("Reactor 标题栏：书签（stub）");
        }),
        button("设置").on_click(|| {
            dispatch_user_action(PopupUserAction::OpenSettings);
        }),
        // 最小化：路径 R 无独立 hide 动作，与关闭同为 Close（宿主 hide 弹窗）
        button("最小化").on_click(|| {
            dispatch_user_action(PopupUserAction::Close);
        }),
        button("关闭").on_click(|| {
            dispatch_user_action(PopupUserAction::Close);
        }),
    ))
    .spacing(6.0)
    .into()
}

fn status_bar(vm: &PopupViewModel) -> Element {
    let status = footer_status_label(vm.is_translating, &vm.source_text);
    let count = source_char_count(&vm.source_text);
    // 翻译中 → 取消；否则 → 重试（整批，service_instance_id: None）
    let action_btn = if vm.is_translating {
        accent_button("取消").on_click(|| {
            dispatch_user_action(PopupUserAction::CancelTranslation);
        })
    } else {
        accent_button("重试").on_click(|| {
            dispatch_user_action(PopupUserAction::Retry {
                service_instance_id: None,
            });
        })
    };
    hstack((
        caption(status.to_string()),
        caption(format!("{count} 字")),
        action_btn,
    ))
    .spacing(8.0)
    .into()
}

fn card_status_label(status: &PopupCardStatus) -> &'static str {
    match status {
        PopupCardStatus::Pending => "等待中",
        PopupCardStatus::Translating => "翻译中",
        PopupCardStatus::Finished => "",
        PopupCardStatus::Failed => "失败",
        PopupCardStatus::Cancelled => "已取消",
    }
}

fn card_tokens_label(card: &PopupCardVm) -> String {
    let has_usage = card.usage_input.is_some() || card.usage_output.is_some();
    if !should_show_tokens(&card.protocol, has_usage) {
        return String::new();
    }
    match (card.usage_input, card.usage_output) {
        (Some(i), Some(o)) => format!("↑{i} ↓{o}"),
        (Some(i), None) => format!("↑{i}"),
        (None, Some(o)) => format!("↓{o}"),
        _ => String::new(),
    }
}

/// 单服务结果卡：服务名、状态、正文/错误、model、tokens、复制。
fn result_card(card: &PopupCardVm) -> Element {
    let status_label = card_status_label(&card.status);
    let model = display_model_name(&card.protocol, &card.model_name);
    let tokens = card_tokens_label(card);
    let body = card_body_text(card);
    let sid = card.service_instance_id.clone();
    let name = if card.service_name.is_empty() {
        "服务".to_string()
    } else {
        card.service_name.clone()
    };

    vstack((
        hstack((
            text_block(name).font_size(13.0).semibold(),
            caption(status_label.to_string()),
        ))
        .spacing(8.0),
        text_block(body).font_size(14.0).wrap().selectable(),
        hstack((
            caption(model),
            caption(tokens),
            button("复制").on_click(move || {
                if sid.is_empty() {
                    log::debug!("复制：无服务实例 id，忽略");
                    return;
                }
                dispatch_user_action(PopupUserAction::CopyResult {
                    service_instance_id: sid.clone(),
                });
            }),
        ))
        .spacing(8.0),
    ))
    .spacing(6.0)
    .into()
}

/// 多服务结果列表（保序）；`scroll_viewer` 包裹结果区（总高可调，见 host inner_size）。
fn results_list(vm: &PopupViewModel) -> Element {
    let cards: Vec<Element> = if vm.cards.is_empty() {
        vec![text_block("（等待结果）").font_size(14.0).into()]
    } else {
        vm.cards.iter().map(result_card).collect()
    };
    scroll_viewer(vstack(cards).spacing(10.0))
        .max_height(RESULTS_SCROLL_MAX_HEIGHT)
        .into()
}

/// 源 / 目标语言 ComboBox：`lang_codes_for_side` 列表；选中后
/// `SetSessionLanguages`（目标侧无 auto）。
fn lang_combo(is_source: bool, current: &str, peer_lang: String) -> ComboBox {
    let codes = lang_codes_for_side(is_source);
    let labels: Vec<String> = codes
        .iter()
        .map(|c| lang_display_name(c).to_string())
        .collect();
    let selected = codes
        .iter()
        .position(|c| *c == current)
        .map(|i| i as i32)
        .unwrap_or(-1);
    let current_owned = current.to_string();

    ComboBox::new(labels)
        .selected_index(selected)
        .on_selection_changed(move |idx| {
            if idx < 0 {
                return;
            }
            let Some(code) = codes.get(idx as usize) else {
                return;
            };
            // 与当前一致则忽略（避免 set selected / 重渲时重复重译）
            if *code == current_owned.as_str() {
                return;
            }
            let (source_lang, target_lang) = if is_source {
                ((*code).to_string(), peer_lang.clone())
            } else {
                (peer_lang.clone(), (*code).to_string())
            };
            dispatch_user_action(PopupUserAction::SetSessionLanguages {
                source_lang,
                target_lang,
            });
        })
        .min_width(140.0)
}

/// 语言栏：源语言 ComboBox + ⇄ 交换 + 目标语言 ComboBox。
fn language_bar(vm: &PopupViewModel) -> Element {
    let source_lang = vm.source_lang.clone();
    let target_lang = vm.target_lang.clone();

    let src_combo = lang_combo(true, &source_lang, target_lang.clone());
    let tgt_combo = lang_combo(false, &target_lang, source_lang.clone());

    let swap_src = source_lang;
    let swap_tgt = target_lang;
    hstack((
        src_combo,
        button("⇄").on_click(move || {
            let (s, t) = swap_session_langs(&swap_src, &swap_tgt);
            dispatch_user_action(PopupUserAction::SetSessionLanguages {
                source_lang: s,
                target_lang: t,
            });
        }),
        tgt_combo,
    ))
    .spacing(8.0)
    .into()
}

/// 渲染翻译弹窗（标题栏 + 源文 + 语言栏 + 多服务结果卡 + 取消/重试 + 状态栏）。
pub fn render_popup(vm: &PopupViewModel) -> Element {
    let source = if vm.source_text.is_empty() {
        String::from("（无源文）")
    } else {
        vm.source_text.clone()
    };

    vstack((
        title_bar(),
        // 源文：只读展示 + 可选中复制（selectable text_block）
        text_block(source).font_size(16.0).wrap().selectable(),
        language_bar(vm),
        results_list(vm),
        status_bar(vm),
    ))
    .spacing(12.0)
    .width(POPUP_VIEW_WIDTH)
    .padding(12.0)
    .into()
}

fn card_body_text(card: &PopupCardVm) -> String {
    let t = card.text.trim();
    if !t.is_empty() {
        return t.to_string();
    }
    let e = card.error_message.trim();
    if !e.is_empty() {
        return e.to_string();
    }
    match card.status {
        PopupCardStatus::Pending => "等待…".into(),
        PopupCardStatus::Translating => "翻译中…".into(),
        PopupCardStatus::Finished => String::new(),
        PopupCardStatus::Failed => "失败".into(),
        PopupCardStatus::Cancelled => "已取消".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(status: PopupCardStatus, text: &str, err: &str) -> PopupCardVm {
        PopupCardVm {
            service_instance_id: "s1".into(),
            service_name: "Mock".into(),
            service_type: "llm".into(),
            protocol: "mock".into(),
            model_name: "m".into(),
            status,
            text: text.into(),
            error_message: err.into(),
            usage_input: None,
            usage_output: None,
            detected_source_lang: None,
        }
    }

    #[test]
    fn view_card_body_prefers_text_then_error() {
        assert_eq!(
            card_body_text(&card(PopupCardStatus::Finished, "hello", "")),
            "hello"
        );
        assert_eq!(
            card_body_text(&card(PopupCardStatus::Failed, "", "timeout")),
            "timeout"
        );
        assert_eq!(
            card_body_text(&card(PopupCardStatus::Pending, "", "")),
            "等待…"
        );
    }

    #[test]
    fn view_footer_status_label_by_state() {
        assert_eq!(footer_status_label(true, ""), "翻译中…");
        assert_eq!(footer_status_label(true, "hello"), "翻译中…");
        assert_eq!(footer_status_label(false, ""), "就绪");
        assert_eq!(footer_status_label(false, "   "), "就绪");
        assert_eq!(footer_status_label(false, "hello"), "完成");
    }

    #[test]
    fn view_source_char_count_uses_unicode_scalars() {
        assert_eq!(source_char_count(""), 0);
        assert_eq!(source_char_count("hello"), 5);
        assert_eq!(source_char_count("柿子"), 2);
        assert_eq!(source_char_count("a😀b"), 3);
    }

    #[test]
    fn view_render_popup_returns_element() {
        let vm = PopupViewModel {
            source_text: "hello".into(),
            cards: vec![card(PopupCardStatus::Finished, "world", "")],
            ..Default::default()
        };
        let _el = render_popup(&vm);
    }

    #[test]
    fn view_render_popup_multi_cards_returns_element() {
        let mut c1 = card(PopupCardStatus::Finished, "one", "");
        c1.service_instance_id = "s1".into();
        c1.service_name = "A".into();
        c1.usage_input = Some(10);
        c1.usage_output = Some(20);
        let mut c2 = card(PopupCardStatus::Failed, "", "timeout");
        c2.service_instance_id = "s2".into();
        c2.service_name = "B".into();
        c2.protocol = "microsoft_edge".into();
        let mut c3 = card(PopupCardStatus::Translating, "", "");
        c3.service_instance_id = "s3".into();
        c3.service_name = "C".into();
        let vm = PopupViewModel {
            source_text: "hello".into(),
            is_translating: true,
            cards: vec![c1, c2, c3],
            source_lang: "en".into(),
            target_lang: "zh-CN".into(),
            ..Default::default()
        };
        let _el = render_popup(&vm);
    }

    #[test]
    fn view_card_status_and_tokens_helpers() {
        assert_eq!(card_status_label(&PopupCardStatus::Pending), "等待中");
        assert_eq!(card_status_label(&PopupCardStatus::Finished), "");
        let mut llm = card(PopupCardStatus::Finished, "ok", "");
        llm.usage_input = Some(1);
        llm.usage_output = Some(2);
        assert_eq!(card_tokens_label(&llm), "↑1 ↓2");
        let mut mt = card(PopupCardStatus::Finished, "ok", "");
        mt.protocol = "microsoft_edge".into();
        mt.usage_input = Some(1);
        assert_eq!(card_tokens_label(&mt), "");
        assert_eq!(display_model_name("openai_chat", "gpt-4o"), "gpt-4o");
        assert_eq!(display_model_name("microsoft_edge", "x"), "");
    }

    #[test]
    fn view_popup_width_is_468() {
        assert!((POPUP_VIEW_WIDTH - 468.0).abs() < f64::EPSILON);
    }

    #[test]
    fn view_accent_is_persimmon_orange() {
        assert_eq!(ACCENT_PERSIMMON, Color::rgb(0xD5, 0x5A, 0x1F));
        assert_eq!(ACCENT_ON_PERSIMMON, Color::rgb(0xFF, 0xFF, 0xFF));
        assert!((RESULTS_SCROLL_MAX_HEIGHT - 360.0).abs() < f64::EPSILON);
    }
}

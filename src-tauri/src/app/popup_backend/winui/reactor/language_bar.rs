//! 语言栏：源/目标 ComboBox + 交换（系统 Combo 抛光）。

#![cfg(all(windows, feature = "popup-winui"))]

use windows_reactor::{
    button, hstack, ComboBox, Element, ElementExt, Symbol, Thickness,
};

use super::dispatch::dispatch_user_action;
use super::langs::{lang_codes_for_side, lang_display_name, swap_session_langs};
use super::tokens::ACCENT_PERSIMMON;
use crate::app::popup_backend::types::{PopupUserAction, PopupViewModel};

/// 源 / 目标语言 ComboBox。
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
        .min_width(148.0)
}

pub fn language_bar(vm: &PopupViewModel) -> Element {
    let source_lang = vm.source_lang.clone();
    let target_lang = vm.target_lang.clone();

    let src_combo = lang_combo(true, &source_lang, target_lang.clone());
    let tgt_combo = lang_combo(false, &target_lang, source_lang.clone());

    let swap_src = source_lang;
    let swap_tgt = target_lang;
    hstack((
        src_combo,
        button("")
            .icon(Symbol::Switch)
            .subtle()
            .automation_name("交换语言")
            .on_click(move || {
                let (s, t) = swap_session_langs(&swap_src, &swap_tgt);
                dispatch_user_action(PopupUserAction::SetSessionLanguages {
                    source_lang: s,
                    target_lang: t,
                });
            })
            .foreground(ACCENT_PERSIMMON),
        tgt_combo,
    ))
    .spacing(8.0)
    .padding(Thickness::xy(0.0, 2.0))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_bar_renders() {
        let vm = PopupViewModel {
            source_lang: "auto".into(),
            target_lang: "zh-CN".into(),
            ..Default::default()
        };
        let _ = language_bar(&vm);
    }
}

use std::sync::{Arc, Mutex};

use crate::app::shortcuts::ShortcutBindingError;
use crate::core::capture::CapturedImage;
use crate::core::config::ConfigStore;
use crate::core::translation::TranslationInput;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppState {
    pub config_store: ConfigStore,
    pending_source_text: Arc<Mutex<Option<String>>>,
    translation_busy: Arc<Mutex<bool>>,
    // capture 流程独立锁：从 start_translation_from_ocr 抓帧到 submit/cancel 释放，
    // 期间挡住二次 Alt+O 覆盖 pending_capture。与 translation_busy 解耦——
    // translation_busy 在 start_translation_from_input 末尾才置位，无法保护 OCR/recognize 窗口。
    capture_in_progress: Arc<Mutex<bool>>,
    // overlay 截图链路：抓到的整屏帧 + 显示器 scale_factor，等待框选裁剪。
    pending_capture: Arc<Mutex<Option<(CapturedImage, f64)>>>,
    // 当前翻译的取消信号。begin 时存入，翻译自然结束 clear、用户取消 cancel。
    // cancel 取出并触发；幂等：无 token 或已清空返回 Ok 无操作。
    current_cancel_token: Arc<Mutex<Option<CancellationToken>>>,
    // 最近一次成功开始的翻译输入，供重试复用。begin 成功后存入，retry 时 take。
    last_translation_input: Arc<Mutex<Option<TranslationInput>>>,
    // 启动时快捷键注册失败的冲突列表。best-effort 注册后，被其他应用占用的
    // 快捷键记录于此，供设置页拉取展示；保存配置全量成功后清空。
    shortcut_conflicts: Arc<Mutex<Vec<ShortcutBindingError>>>,
}

impl AppState {
    pub fn new(config_store: ConfigStore) -> Self {
        Self {
            config_store,
            pending_source_text: Arc::new(Mutex::new(None)),
            translation_busy: Arc::new(Mutex::new(false)),
            capture_in_progress: Arc::new(Mutex::new(false)),
            pending_capture: Arc::new(Mutex::new(None)),
            current_cancel_token: Arc::new(Mutex::new(None)),
            last_translation_input: Arc::new(Mutex::new(None)),
            shortcut_conflicts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn set_pending_source_text(&self, text: String) -> Result<(), String> {
        let mut pending = self
            .pending_source_text
            .lock()
            .map_err(|_| "原文状态锁已损坏".to_string())?;
        *pending = Some(text);
        Ok(())
    }

    pub fn take_pending_source_text(&self) -> Result<Option<String>, String> {
        let mut pending = self
            .pending_source_text
            .lock()
            .map_err(|_| "原文状态锁已损坏".to_string())?;
        Ok(pending.take())
    }

    pub fn try_begin_translation(&self) -> Result<(), String> {
        let mut busy = self
            .translation_busy
            .lock()
            .map_err(|_| "翻译状态锁已损坏".to_string())?;
        if *busy {
            return Err("正在翻译中，请稍后再试".to_string());
        }
        *busy = true;
        Ok(())
    }

    pub fn finish_translation(&self) -> Result<(), String> {
        let mut busy = self
            .translation_busy
            .lock()
            .map_err(|_| "翻译状态锁已损坏".to_string())?;
        *busy = false;
        Ok(())
    }

    pub fn is_translation_busy(&self) -> bool {
        self.translation_busy
            .lock()
            .map(|busy| *busy)
            .unwrap_or(false)
    }

    /// 占住 capture 锁。overlay 截图链路从抓帧到 submit/cancel 期间持锁，
    /// 挡住二次 Alt+O 覆盖 pending_capture。失败表示已有 capture 在进行。
    pub fn try_begin_capture(&self) -> Result<(), String> {
        let mut busy = self
            .capture_in_progress
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_string())?;
        if *busy {
            return Err("正在截图或识别中，请稍后再试".to_string());
        }
        *busy = true;
        Ok(())
    }

    /// 释放 capture 锁。幂等：对已清位再清无害。submit/cancel 各分支均调此释放。
    pub fn finish_capture(&self) -> Result<(), String> {
        let mut busy = self
            .capture_in_progress
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_string())?;
        *busy = false;
        Ok(())
    }

    pub fn set_pending_capture(&self, frame: CapturedImage, scale_factor: f64) -> Result<(), String> {
        let mut slot = self
            .pending_capture
            .lock()
            .map_err(|_| "截图帧状态锁已损坏".to_string())?;
        *slot = Some((frame, scale_factor));
        Ok(())
    }

    pub fn pending_capture_meta(&self) -> Result<Option<(u32, u32, f64)>, String> {
        let slot = self
            .pending_capture
            .lock()
            .map_err(|_| "截图帧状态锁已损坏".to_string())?;
        Ok(slot.as_ref().map(|(frame, scale)| (frame.width, frame.height, *scale)))
    }

    pub fn pending_capture_bytes(&self) -> Result<Option<Vec<u8>>, String> {
        let slot = self
            .pending_capture
            .lock()
            .map_err(|_| "截图帧状态锁已损坏".to_string())?;
        Ok(slot.as_ref().map(|(frame, _)| frame.bytes.clone()))
    }

    pub fn take_pending_capture(&self) -> Result<Option<(CapturedImage, f64)>, String> {
        let mut slot = self
            .pending_capture
            .lock()
            .map_err(|_| "截图帧状态锁已损坏".to_string())?;
        Ok(slot.take())
    }

    pub fn set_current_cancel_token(&self, token: CancellationToken) -> Result<(), String> {
        let mut slot = self
            .current_cancel_token
            .lock()
            .map_err(|_| "取消信号状态锁已损坏".to_string())?;
        *slot = Some(token);
        Ok(())
    }

    pub fn cancel_current_translation(&self) -> Result<(), String> {
        let token = {
            let mut slot = self
                .current_cancel_token
                .lock()
                .map_err(|_| "取消信号状态锁已损坏".to_string())?;
            slot.take()
        };
        if let Some(token) = token {
            token.cancel();
        }
        Ok(())
    }

    pub fn set_last_translation_input(&self, input: TranslationInput) -> Result<(), String> {
        let mut slot = self
            .last_translation_input
            .lock()
            .map_err(|_| "重试输入状态锁已损坏".to_string())?;
        *slot = Some(input);
        Ok(())
    }

    pub fn take_last_translation_input(&self) -> Result<Option<TranslationInput>, String> {
        let mut slot = self
            .last_translation_input
            .lock()
            .map_err(|_| "重试输入状态锁已损坏".to_string())?;
        Ok(slot.take())
    }

    pub fn clear_current_cancel_token(&self) -> Result<(), String> {
        let mut slot = self
            .current_cancel_token
            .lock()
            .map_err(|_| "取消信号状态锁已损坏".to_string())?;
        *slot = None;
        Ok(())
    }

    pub fn set_shortcut_conflicts(
        &self,
        conflicts: Vec<ShortcutBindingError>,
    ) -> Result<(), String> {
        let mut slot = self
            .shortcut_conflicts
            .lock()
            .map_err(|_| "快捷键冲突状态锁已损坏".to_string())?;
        *slot = conflicts;
        Ok(())
    }

    pub fn shortcut_conflicts(&self) -> Result<Vec<ShortcutBindingError>, String> {
        let slot = self
            .shortcut_conflicts
            .lock()
            .map_err(|_| "快捷键冲突状态锁已损坏".to_string())?;
        Ok(slot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::AppConfig;
    use std::{
        path::PathBuf,
        sync::{Arc, RwLock},
    };

    fn app_state() -> AppState {
        AppState::new(ConfigStore::from_parts_for_test(
            PathBuf::from("unused-config.json"),
            Arc::new(RwLock::new(AppConfig::from_env())),
        ))
    }

    #[test]
    fn pending_source_text_is_consumed_once() {
        let state = app_state();

        state
            .set_pending_source_text("hello".to_string())
            .expect("写入待回填原文");

        assert_eq!(
            state.take_pending_source_text().expect("读取待回填原文"),
            Some("hello".to_string())
        );
        assert_eq!(
            state
                .take_pending_source_text()
                .expect("再次读取待回填原文"),
            None
        );
    }

    #[test]
    fn translation_busy_rejects_second_begin_until_finished() {
        let state = app_state();

        state.try_begin_translation().expect("开始第一次翻译");
        assert!(state.try_begin_translation().is_err());

        state.finish_translation().expect("结束翻译");
        state.try_begin_translation().expect("结束后可再次开始");
    }

    #[test]
    fn pending_capture_frame_round_trips() {
        use crate::core::capture::{CapturedImage, CapturedImageFormat};
        let state = app_state();
        let frame = CapturedImage {
            bytes: vec![1, 2, 3, 4],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        };

        state.set_pending_capture(frame.clone(), 1.5).expect("写入截图帧");

        let meta = state.pending_capture_meta().expect("读取 meta").expect("应有 meta");
        assert_eq!(meta, (1, 1, 1.5));

        assert_eq!(state.pending_capture_bytes().expect("读取 bytes").as_deref(), Some(&[1, 2, 3, 4][..]));

        let taken = state.take_pending_capture().expect("取出帧").expect("应有帧");
        assert_eq!(taken.0, frame);
        assert_eq!(taken.1, 1.5);

        assert!(state.take_pending_capture().expect("再次取出").is_none());
    }

    #[test]
    fn pending_capture_overwrites_previous() {
        use crate::core::capture::{CapturedImage, CapturedImageFormat};
        let state = app_state();
        let first = CapturedImage {
            bytes: vec![9, 9, 9, 9],
            width: 1,
            height: 1,
            format: CapturedImageFormat::Bgra8,
        };
        let second = CapturedImage {
            bytes: vec![7, 7, 7, 7],
            width: 2,
            height: 2,
            format: CapturedImageFormat::Bgra8,
        };

        state.set_pending_capture(first, 1.0).expect("写入第一帧");
        state.set_pending_capture(second.clone(), 2.0).expect("覆盖第二帧");

        let taken = state.take_pending_capture().expect("取出").expect("应有帧");
        assert_eq!(taken.0, second);
        assert_eq!(taken.1, 2.0);
    }

    #[test]
    fn is_translation_busy_reflects_begin_and_finish() {
        let state = app_state();

        assert!(!state.is_translation_busy(), "初始不应处于 busy");

        state.try_begin_translation().expect("开始翻译");
        assert!(state.is_translation_busy(), "begin 后应处于 busy");

        state.finish_translation().expect("结束翻译");
        assert!(!state.is_translation_busy(), "finish 后应退出 busy");
    }

    #[test]
    fn try_begin_capture_rejects_second_begin_until_finished() {
        let state = app_state();

        state.try_begin_capture().expect("开始第一次截图");
        assert!(
            state.try_begin_capture().is_err(),
            "capture 进行中应拒绝二次 begin"
        );

        state.finish_capture().expect("结束截图");
        state.try_begin_capture().expect("结束后可再次开始");
        state.finish_capture().expect("再次结束");
    }

    #[test]
    fn finish_capture_is_idempotent() {
        let state = app_state();

        state.try_begin_capture().expect("开始截图");
        state.finish_capture().expect("第一次释放");
        // 对已清位再清应无害（cancel/submit 各分支可能重复释放）。
        state.finish_capture().expect("幂等释放");
    }

    #[test]
    fn cancel_token_triggers_on_cancel_current_translation() {
        let state = app_state();
        let token = tokio_util::sync::CancellationToken::new();
        state.set_current_cancel_token(token.clone()).expect("写入 cancel token");

        state.cancel_current_translation().expect("触发取消");

        assert!(token.is_cancelled(), "token 应被触发");
    }

    #[test]
    fn cancel_current_translation_is_idempotent_when_no_token() {
        let state = app_state();
        state.cancel_current_translation().expect("无 token 取消应幂等");
    }

    #[test]
    fn cancel_current_translation_is_idempotent_after_take() {
        let state = app_state();
        let token = tokio_util::sync::CancellationToken::new();
        state.set_current_cancel_token(token.clone()).expect("写入 cancel token");

        state.cancel_current_translation().expect("第一次取消触发");
        state.cancel_current_translation().expect("重复取消应幂等");

        assert!(token.is_cancelled());
    }

    #[test]
    fn last_translation_input_round_trips() {
        use crate::core::translation::TranslationInput;
        let state = app_state();
        let input = TranslationInput::ManualText("hello".to_string());

        state.set_last_translation_input(input.clone()).expect("写入重试输入");

        let taken = state.take_last_translation_input().expect("取出重试输入");
        assert_eq!(taken, Some(input));

        let again = state.take_last_translation_input().expect("再次取出");
        assert_eq!(again, None);
    }

    #[test]
    fn last_translation_input_overwrites_previous() {
        use crate::core::translation::TranslationInput;
        let state = app_state();
        let first = TranslationInput::SelectedText("first".to_string());
        let second = TranslationInput::SelectedText("second".to_string());

        state.set_last_translation_input(first).expect("写入第一个");
        state.set_last_translation_input(second.clone()).expect("覆盖第二个");

        let taken = state.take_last_translation_input().expect("取出");
        assert_eq!(taken, Some(second));
    }

    #[test]
    fn clear_current_cancel_token_is_idempotent() {
        let state = app_state();
        let token = tokio_util::sync::CancellationToken::new();
        state.set_current_cancel_token(token).expect("写入 cancel token");

        state.clear_current_cancel_token().expect("第一次清空");
        state.clear_current_cancel_token().expect("幂等清空");
        state.cancel_current_translation().expect("清空后取消应幂等");
    }
}

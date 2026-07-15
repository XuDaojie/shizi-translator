use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use crate::app::shortcuts::ShortcutBindingError;
use crate::core::capture::CapturedImage;
use crate::core::config::ConfigStore;
use crate::core::history::HistoryStore;
use crate::core::mt::EdgeTranslateEnv;
use crate::core::translation::TranslationInput;
use tokio_util::sync::CancellationToken;

/// 截图框选提交后的用途：翻译链路 vs 纯识别（OCR 窗）。
/// 由入口在 `try_begin_capture` 成功后设置，`submit_capture_region` 按此分叉。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapturePurpose {
    #[default]
    Translate,
    RecognizeOnly,
}

#[derive(Clone)]
pub struct AppState {
    pub config_store: ConfigStore,
    pub history_store: HistoryStore,
    pending_source_text: Arc<Mutex<Option<String>>>,
    translation_busy: Arc<Mutex<bool>>,
    // 翻译代次：每次 begin_translation_overriding 递增，用于区分"当前翻译"与"已被
    // 接管的旧翻译"。spawn 收尾凭 generation 判断是否仍为当前翻译，避免旧翻译收尾
    // 清掉新翻译的 cancel token / 释放 busy。
    translation_generation: Arc<Mutex<u64>>,
    // capture 流程独立锁：从抓帧到 submit/cancel 释放，期间挡住二次截图快捷键
    // 覆盖 pending_capture。与 translation_busy 解耦——translation_busy 在
    // start_translation_from_input 末尾才置位，无法保护 OCR/recognize 窗口。
    capture_in_progress: Arc<Mutex<bool>>,
    // overlay 提交用途：Translate → 翻译弹窗；RecognizeOnly → OCR 窗事件。
    capture_purpose: Arc<Mutex<CapturePurpose>>,
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
    // 会话语言（运行时内存态）：启动从 config 初始化，前端 set_session_languages
    // 写入，所有翻译入口经 start_translation_from_input 读取。不持久化，重启重置。
    // 存语言代码（如 "auto"/"zh-CN"），非显示名。
    session_source_lang: Arc<Mutex<String>>,
    session_target_lang: Arc<Mutex<String>>,
    // WebView 初始化采集的浏览器环境信息（UA/Accept-Language），供微软翻译拼装请求头。
    // 进程级内存，不持久化；每次启动由前端 main 窗口重新采集写入。
    edge_translate_env: Arc<Mutex<Option<EdgeTranslateEnv>>>,
    interface_language_revision: Arc<AtomicU64>,
}

impl AppState {
    pub fn new(config_store: ConfigStore, history_store: HistoryStore) -> Self {
        let default_source_lang = config_store
            .get()
            .map(|c| c.default_source_lang)
            .unwrap_or_else(|_| "auto".to_string());
        let default_target_lang = config_store
            .get()
            .map(|c| c.target_lang)
            .unwrap_or_else(|_| "zh-CN".to_string());
        Self {
            config_store,
            history_store,
            pending_source_text: Arc::new(Mutex::new(None)),
            translation_busy: Arc::new(Mutex::new(false)),
            translation_generation: Arc::new(Mutex::new(0)),
            capture_in_progress: Arc::new(Mutex::new(false)),
            capture_purpose: Arc::new(Mutex::new(CapturePurpose::Translate)),
            pending_capture: Arc::new(Mutex::new(None)),
            current_cancel_token: Arc::new(Mutex::new(None)),
            last_translation_input: Arc::new(Mutex::new(None)),
            shortcut_conflicts: Arc::new(Mutex::new(Vec::new())),
            session_source_lang: Arc::new(Mutex::new(default_source_lang)),
            session_target_lang: Arc::new(Mutex::new(default_target_lang)),
            edge_translate_env: Arc::new(Mutex::new(None)),
            interface_language_revision: Arc::new(AtomicU64::new(0)),
        }
    }

    #[cfg(test)]
    pub fn new_for_test(config_store: ConfigStore) -> Self {
        Self::new(
            config_store,
            HistoryStore::in_memory_for_test().expect("创建内存历史存储"),
        )
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

    /// 强制开始一次翻译并登记取消信号。若有翻译进行中，原子地触发其取消信号，
    /// 使旧 spawn 收尾（`finish_translation_if_current`）因 generation 不匹配而不再
    /// 触碰共享状态。返回本次翻译的 generation 号，spawn 任务收尾凭此判断是否仍为当前翻译。
    ///
    /// 与旧 `try_begin_translation` 的区别：不再因 busy 拒绝新翻译，而是让新翻译
    /// 接管——最新输入优先级最高，旧翻译被中断。
    pub fn begin_translation_overriding(
        &self,
        cancel_token: CancellationToken,
    ) -> Result<u64, String> {
        let mut busy = self
            .translation_busy
            .lock()
            .map_err(|_| "翻译状态锁已损坏".to_string())?;
        let mut generation = self
            .translation_generation
            .lock()
            .map_err(|_| "翻译代次状态锁已损坏".to_string())?;
        {
            let mut token_slot = self
                .current_cancel_token
                .lock()
                .map_err(|_| "取消信号状态锁已损坏".to_string())?;
            if *busy {
                // 有翻译进行中：取出并触发其取消信号，让旧 spawn 尽快收尾。
                // 收尾时 generation 已递增，finish_translation_if_current 判定非当前而不释放 busy。
                if let Some(token) = token_slot.take() {
                    token.cancel();
                }
            }
            *token_slot = Some(cancel_token);
        }
        *generation += 1;
        *busy = true;
        Ok(*generation)
    }

    /// spawn 收尾：仅当 generation 仍为当前翻译时才释放 busy 与 cancel token。
    /// 已被新翻译接管（generation 不匹配）时直接返回，避免清掉新翻译的状态。
    pub fn finish_translation_if_current(&self, generation: u64) -> Result<(), String> {
        {
            let mut busy = self
                .translation_busy
                .lock()
                .map_err(|_| "翻译状态锁已损坏".to_string())?;
            let current_generation = self
                .translation_generation
                .lock()
                .map_err(|_| "翻译代次状态锁已损坏".to_string())?;
            if *current_generation != generation {
                return Ok(());
            }
            *busy = false;
        }
        self.clear_current_cancel_token()?;
        Ok(())
    }

    pub fn is_translation_busy(&self) -> bool {
        self.translation_busy
            .lock()
            .map(|busy| *busy)
            .unwrap_or(false)
    }

    /// 占住 capture 锁。overlay 截图链路从抓帧到 submit/cancel 期间持锁，
    /// 挡住二次截图快捷键覆盖 pending_capture。失败表示已有 capture 在进行。
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

    /// 设置截图提交用途。入口在 try_begin_capture 成功后调用。
    pub fn set_capture_purpose(&self, purpose: CapturePurpose) -> Result<(), String> {
        let mut slot = self
            .capture_purpose
            .lock()
            .map_err(|_| "截图用途状态锁已损坏".to_string())?;
        *slot = purpose;
        Ok(())
    }

    /// 读截图提交用途。锁毒化回退 Translate。
    pub fn capture_purpose(&self) -> CapturePurpose {
        self.capture_purpose
            .lock()
            .map(|slot| *slot)
            .unwrap_or(CapturePurpose::Translate)
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

    /// 读会话源/目标语言。锁毒化回退 ("auto", "zh-CN")，不返回 Err
    pub fn session_languages(&self) -> (String, String) {
        let source = self
            .session_source_lang
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "auto".to_string());
        let target = self
            .session_target_lang
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "zh-CN".to_string());
        (source, target)
    }

    /// 写会话源/目标语言。锁毒化返回 Err。不持久化。
    pub fn set_session_languages(
        &self,
        source: String,
        target: String,
    ) -> Result<(), String> {
        let mut s = self
            .session_source_lang
            .lock()
            .map_err(|_| "会话源语言锁已损坏".to_string())?;
        let mut t = self
            .session_target_lang
            .lock()
            .map_err(|_| "会话目标语言锁已损坏".to_string())?;
        *s = source;
        *t = target;
        Ok(())
    }

    /// 写入前端采集的浏览器环境信息。锁毒化返回 Err。不持久化。
    pub fn set_edge_translate_env(&self, env: EdgeTranslateEnv) -> Result<(), String> {
        let mut slot = self
            .edge_translate_env
            .lock()
            .map_err(|_| "Edge 翻译环境锁已损坏".to_string())?;
        *slot = Some(env);
        Ok(())
    }

    /// 读浏览器环境信息（clone）。锁毒化返回 None，不返回 Err。
    pub fn edge_translate_env(&self) -> Option<EdgeTranslateEnv> {
        self.edge_translate_env
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or(None)
    }

    pub fn next_interface_language_revision(&self) -> u64 {
        self.interface_language_revision
            .fetch_add(1, Ordering::SeqCst)
            + 1
    }

    pub fn interface_language_revision(&self) -> u64 {
        self.interface_language_revision.load(Ordering::SeqCst)
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
        AppState::new_for_test(ConfigStore::from_parts_for_test(
            PathBuf::from("unused-config.json"),
            Arc::new(RwLock::new(AppConfig::default())),
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
    fn begin_translation_overriding_takes_over_in_progress_translation() {
        let state = app_state();
        let old_token = CancellationToken::new();
        state
            .begin_translation_overriding(old_token.clone())
            .expect("旧翻译开始");
        assert!(!old_token.is_cancelled(), "接管前旧 token 不应触发");

        let new_gen = state
            .begin_translation_overriding(CancellationToken::new())
            .expect("新翻译接管");
        assert!(old_token.is_cancelled(), "接管应触发旧 token 取消");
        assert!(state.is_translation_busy(), "接管后仍处于 busy");
        assert!(new_gen >= 2, "generation 应递增");
    }

    #[test]
    fn finish_if_current_ignored_when_generation_stale() {
        let state = app_state();
        let old_gen = state
            .begin_translation_overriding(CancellationToken::new())
            .expect("旧翻译开始");
        let new_gen = state
            .begin_translation_overriding(CancellationToken::new())
            .expect("新翻译接管");

        state
            .finish_translation_if_current(old_gen)
            .expect("旧翻译收尾");
        assert!(
            state.is_translation_busy(),
            "旧翻译收尾不应释放 busy"
        );

        state
            .finish_translation_if_current(new_gen)
            .expect("新翻译收尾");
        assert!(!state.is_translation_busy(), "新翻译收尾应释放 busy");
    }

    #[test]
    fn stale_finish_does_not_clear_new_cancel_token() {
        let state = app_state();
        let old_gen = state
            .begin_translation_overriding(CancellationToken::new())
            .expect("旧翻译");
        let new_token = CancellationToken::new();
        let _new_gen = state
            .begin_translation_overriding(new_token.clone())
            .expect("新翻译接管");

        // 旧翻译 spawn 收尾：generation 不匹配，不应触碰新 token
        state
            .finish_translation_if_current(old_gen)
            .expect("旧收尾");

        state.cancel_current_translation().expect("取消");
        assert!(
            new_token.is_cancelled(),
            "新 token 不应被旧收尾清除"
        );
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

        let gen = state
            .begin_translation_overriding(CancellationToken::new())
            .expect("开始翻译");
        assert!(state.is_translation_busy(), "begin 后应处于 busy");

        state
            .finish_translation_if_current(gen)
            .expect("结束翻译");
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
    fn capture_purpose_defaults_to_translate_and_round_trips() {
        let state = app_state();
        assert_eq!(state.capture_purpose(), CapturePurpose::Translate);

        state
            .set_capture_purpose(CapturePurpose::RecognizeOnly)
            .expect("设为纯识别");
        assert_eq!(state.capture_purpose(), CapturePurpose::RecognizeOnly);

        state
            .set_capture_purpose(CapturePurpose::Translate)
            .expect("设回翻译");
        assert_eq!(state.capture_purpose(), CapturePurpose::Translate);
    }

    #[test]
    fn cancel_token_triggers_on_cancel_current_translation() {
        let state = app_state();
        let token = tokio_util::sync::CancellationToken::new();
        state
            .begin_translation_overriding(token.clone())
            .expect("开始翻译并登记 token");

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
        state
            .begin_translation_overriding(token.clone())
            .expect("开始翻译并登记 token");

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
        state
            .begin_translation_overriding(token)
            .expect("开始翻译并登记 token");

        state.clear_current_cancel_token().expect("第一次清空");
        state.clear_current_cancel_token().expect("幂等清空");
        state.cancel_current_translation().expect("清空后取消应幂等");
    }

    #[test]
    fn session_languages_init_from_config() {
        let mut config = AppConfig::default();
        config.default_source_lang = "en-US".to_string();
        config.target_lang = "ja-JP".to_string();
        let state = AppState::new_for_test(ConfigStore::from_parts_for_test(
            PathBuf::from("unused-config.json"),
            Arc::new(RwLock::new(config)),
        ));
        let (source, target) = state.session_languages();
        assert_eq!(source, "en-US");
        assert_eq!(target, "ja-JP");
    }

    #[test]
    fn set_session_languages_updates_state() {
        let state = app_state();
        state
            .set_session_languages("en-US".to_string(), "zh-CN".to_string())
            .expect("set 应成功");
        let (source, target) = state.session_languages();
        assert_eq!(source, "en-US");
        assert_eq!(target, "zh-CN");
    }

    #[test]
    fn set_session_languages_persists_until_reset() {
        let mut config = AppConfig::default();
        config.target_lang = "zh-CN".to_string();
        let store = Arc::new(RwLock::new(config));
        let state = AppState::new_for_test(ConfigStore::from_parts_for_test(
            PathBuf::from("unused-config.json"),
            store.clone(),
        ));
        state
            .set_session_languages("auto".to_string(), "en-US".to_string())
            .expect("set 应成功");
        // 改 config 的 target_lang
        store.write().unwrap().target_lang = "ja-JP".to_string();
        // 会话语言仍是 set 的值
        let (source, target) = state.session_languages();
        assert_eq!(source, "auto");
        assert_eq!(target, "en-US");
    }

    #[test]
    fn edge_translate_env_round_trips() {
        let state = app_state();
        assert!(state.edge_translate_env().is_none(), "初始应为 None");
        state
            .set_edge_translate_env(crate::core::mt::EdgeTranslateEnv {
                user_agent: "UA".to_string(),
                accept_language: "zh-CN".to_string(),
            })
            .expect("写入 env");
        let env = state.edge_translate_env().expect("读取 env");
        assert_eq!(env.user_agent, "UA");
        assert_eq!(env.accept_language, "zh-CN");
    }

    #[test]
    fn interface_language_revision_is_shared_and_monotonic() {
        let state = app_state();
        let cloned = state.clone();
        assert_eq!(state.interface_language_revision(), 0);
        assert_eq!(state.next_interface_language_revision(), 1);
        assert_eq!(cloned.next_interface_language_revision(), 2);
        assert_eq!(state.interface_language_revision(), 2);
    }
}

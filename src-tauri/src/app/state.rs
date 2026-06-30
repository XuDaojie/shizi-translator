use std::sync::{Arc, Mutex};

use crate::core::capture::CapturedImage;
use crate::core::config::ConfigStore;

#[derive(Clone)]
pub struct AppState {
    pub config_store: ConfigStore,
    pending_source_text: Arc<Mutex<Option<String>>>,
    translation_busy: Arc<Mutex<bool>>,
    // overlay 截图链路：抓到的整屏帧 + 显示器 scale_factor，等待框选裁剪。
    pending_capture: Arc<Mutex<Option<(CapturedImage, f64)>>>,
}

impl AppState {
    pub fn new(config_store: ConfigStore) -> Self {
        Self {
            config_store,
            pending_source_text: Arc::new(Mutex::new(None)),
            translation_busy: Arc::new(Mutex::new(false)),
            pending_capture: Arc::new(Mutex::new(None)),
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

        let taken = state.take_pending_capture().expect("取出帧").expect("应有帧");
        assert_eq!(taken.0, frame);
        assert_eq!(taken.1, 1.5);

        assert!(state.take_pending_capture().expect("再次取出").is_none());
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
}

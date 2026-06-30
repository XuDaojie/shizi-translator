use std::sync::{Arc, Mutex};

use crate::core::config::ConfigStore;

#[derive(Clone)]
pub struct AppState {
    pub config_store: ConfigStore,
    pending_source_text: Arc<Mutex<Option<String>>>,
    translation_busy: Arc<Mutex<bool>>,
}

impl AppState {
    pub fn new(config_store: ConfigStore) -> Self {
        Self {
            config_store,
            pending_source_text: Arc::new(Mutex::new(None)),
            translation_busy: Arc::new(Mutex::new(false)),
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
    fn is_translation_busy_reflects_begin_and_finish() {
        let state = app_state();

        assert!(!state.is_translation_busy(), "初始不应处于 busy");

        state.try_begin_translation().expect("开始翻译");
        assert!(state.is_translation_busy(), "begin 后应处于 busy");

        state.finish_translation().expect("结束翻译");
        assert!(!state.is_translation_busy(), "finish 后应退出 busy");
    }
}

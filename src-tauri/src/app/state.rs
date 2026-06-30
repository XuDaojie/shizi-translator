use std::sync::{Arc, Mutex};

use crate::core::config::ConfigStore;

#[derive(Clone)]
pub struct AppState {
    pub config_store: ConfigStore,
    pending_source_text: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub fn new(config_store: ConfigStore) -> Self {
        Self {
            config_store,
            pending_source_text: Arc::new(Mutex::new(None)),
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
            state.take_pending_source_text().expect("再次读取待回填原文"),
            None
        );
    }
}

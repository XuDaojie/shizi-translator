use crate::core::config::ConfigStore;

#[derive(Clone)]
pub struct AppState {
    pub config_store: ConfigStore,
}

impl AppState {
    pub fn new(config_store: ConfigStore) -> Self {
        Self { config_store }
    }
}

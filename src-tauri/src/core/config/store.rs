use std::{
    fs,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use tauri::Manager;

use super::AppConfig;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("无法获取应用配置目录：{0}")]
    AppConfigDir(String),
    #[error("无法创建应用配置目录：{0}")]
    CreateDir(#[source] std::io::Error),
    #[error("无法读取配置文件：{0}")]
    Read(#[source] std::io::Error),
    #[error("无法解析配置文件：{0}")]
    Parse(#[source] serde_json::Error),
    #[error("无法序列化配置：{0}")]
    Serialize(#[source] serde_json::Error),
    #[error("无法写入配置文件：{0}")]
    Write(#[source] std::io::Error),
    #[error("配置状态锁已损坏")]
    Lock,
}

#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
    config: Arc<RwLock<AppConfig>>,
}

impl ConfigStore {
    #[cfg(test)]
    pub fn from_parts_for_test(path: PathBuf, config: Arc<RwLock<AppConfig>>) -> Self {
        Self { path, config }
    }

    pub fn load(app_handle: &tauri::AppHandle) -> Result<Self, ConfigError> {
        let config_dir = app_handle
            .path()
            .app_config_dir()
            .map_err(|error| ConfigError::AppConfigDir(error.to_string()))?;
        fs::create_dir_all(&config_dir).map_err(ConfigError::CreateDir)?;

        let path = config_dir.join("config.json");
        let config = if path.exists() {
            match fs::read_to_string(&path)
                .map_err(ConfigError::Read)
                .and_then(|contents| serde_json::from_str::<AppConfig>(&contents).map_err(ConfigError::Parse))
            {
                Ok(config) => config.normalized(),
                Err(err) => {
                    log::warn!("配置文件解析失败，使用默认配置：{err}");
                    AppConfig::default()
                }
            }
        } else {
            AppConfig::default()
        };

        Ok(Self {
            path,
            config: Arc::new(RwLock::new(config)),
        })
    }

    pub fn get(&self) -> Result<AppConfig, ConfigError> {
        self.config
            .read()
            .map(|config| config.clone())
            .map_err(|_| ConfigError::Lock)
    }

    pub fn save(&self, config: AppConfig) -> Result<AppConfig, ConfigError> {
        let config = config.normalized();
        let contents = serde_json::to_string_pretty(&config).map_err(ConfigError::Serialize)?;
        fs::write(&self.path, contents).map_err(ConfigError::Write)?;

        let mut current = self.config.write().map_err(|_| ConfigError::Lock)?;
        *current = config.clone();

        Ok(config)
    }
}

use crate::config::types::{AppConfig, GiftRule, RuntimeState};
use crate::config::validation::{validate_app_config, validate_rules, ValidationError};
use std::path::PathBuf;
use tokio::sync::watch;
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Validation(#[from] ValidationError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct ConfigStore {
    data: AppConfig,
    path: PathBuf,
    tx: watch::Sender<AppConfig>,
}

impl ConfigStore {
    pub async fn load_or_default(path: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let path = path.into();
        let default = AppConfig::default();

        let data = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(ConfigError::Io)?;
            let parsed: serde_json::Value = serde_json::from_str(&content)?;
            let default_json = serde_json::to_value(&default)?;
            let merged = deep_merge(&default_json, &parsed);
            let cfg: AppConfig = serde_json::from_value(merged)?;
            validate_app_config(&cfg)?;
            cfg
        } else {
            default
        };

        let (tx, _) = watch::channel(data.clone());
        Ok(Self { data, path, tx })
    }

    pub fn get(&self) -> &AppConfig {
        &self.data
    }

    pub fn sender(&self) -> watch::Sender<AppConfig> {
        self.tx.clone()
    }

    pub async fn update(&mut self, partial: serde_json::Value) -> Result<(), ConfigError> {
        let current_json = serde_json::to_value(&self.data)?;
        let merged = deep_merge(&current_json, &partial);
        let next: AppConfig = serde_json::from_value(merged)?;
        validate_app_config(&next)?;
        self.persist_data(&next).await?;
        self.data = next;
        let _ = self.tx.send(self.data.clone());
        Ok(())
    }

    pub async fn set_rules(&mut self, rules: Vec<GiftRule>) -> Result<(), ConfigError> {
        validate_rules(&rules)?;
        let mut next_data = self.data.clone();
        next_data.rules = rules;
        self.persist_data(&next_data).await?;
        self.data = next_data;
        let _ = self.tx.send(self.data.clone());
        Ok(())
    }

    async fn persist_data(&self, data: &AppConfig) -> Result<(), ConfigError> {
        let content = serde_json::to_string_pretty(data).map_err(ConfigError::Json)?;
        let tmp_path = self.path.with_extension("tmp");
        tokio::fs::write(tmp_path.as_path(), content.as_bytes())
            .await
            .map_err(ConfigError::Io)?;
        tokio::fs::rename(tmp_path.as_path(), self.path.as_path())
            .await
            .map_err(ConfigError::Io)?;
        Ok(())
    }
}

fn deep_merge(base: &serde_json::Value, overlay: &serde_json::Value) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            let mut result = base_map.clone();
            for (key, overlay_val) in overlay_map {
                if let Some(base_val) = base_map.get(key) {
                    if base_val.is_object() && overlay_val.is_object() {
                        result.insert(key.clone(), deep_merge(base_val, overlay_val));
                    } else {
                        result.insert(key.clone(), overlay_val.clone());
                    }
                } else {
                    result.insert(key.clone(), overlay_val.clone());
                }
            }
            serde_json::Value::Object(result)
        }
        _ => overlay.clone(),
    }
}

pub struct RuntimeStateStore {
    data: RuntimeState,
    path: PathBuf,
}

impl RuntimeStateStore {
    pub fn load_or_default(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let data = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(state) => state,
                    Err(e) => {
                        error!("Failed to parse {}: {e}", path.display());
                        RuntimeState::default()
                    }
                },
                Err(e) => {
                    error!("Failed to read {}: {e}", path.display());
                    RuntimeState::default()
                }
            }
        } else {
            RuntimeState::default()
        };
        Self { data, path }
    }

    pub fn open_platform_game_id(&self) -> &str {
        &self.data.open_platform_game_id
    }

    pub async fn set_open_platform_game_id(&mut self, value: String) -> Result<(), ConfigError> {
        self.data.open_platform_game_id = value;
        let content = serde_json::to_string_pretty(&self.data).map_err(ConfigError::Json)?;
        let tmp_path = self.path.with_extension("state.tmp");
        tokio::fs::write(tmp_path.as_path(), content.as_bytes())
            .await
            .map_err(ConfigError::Io)?;
        tokio::fs::rename(tmp_path.as_path(), self.path.as_path())
            .await
            .map_err(ConfigError::Io)?;
        Ok(())
    }
}

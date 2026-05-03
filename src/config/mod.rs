pub mod store;
pub mod types;
pub mod validation;

use crate::config::types::AppConfig;
pub use store::{ConfigError, ConfigStore, RuntimeStateStore};
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Clone)]
pub struct ConfigHandle {
    store: Arc<tokio::sync::Mutex<ConfigStore>>,
    tx: watch::Sender<AppConfig>,
}

impl ConfigHandle {
    pub fn new(store: ConfigStore) -> Self {
        let tx = store.sender();
        Self {
            store: Arc::new(tokio::sync::Mutex::new(store)),
            tx,
        }
    }

    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, ConfigStore> {
        self.store.lock().await
    }

    pub fn snapshot(&self) -> AppConfig {
        self.tx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<AppConfig> {
        self.tx.subscribe()
    }
}

pub use validation::{
    validate_bilibili_start, validate_manual_strength, BilibiliStartInput, ValidationError,
};

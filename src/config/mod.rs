pub mod store;
pub mod types;
pub mod validation;

use crate::config::types::AppConfig;
use std::sync::Arc;
pub use store::{ConfigError, ConfigStore, RuntimeStateStore};
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
}

pub use validation::{
    parse_bilibili_start, parse_manual_strength, BilibiliStartInput, ValidationError,
};

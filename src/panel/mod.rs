use crate::engine::types::PanelEvent;
use tokio::sync::broadcast;

pub struct PanelHub {
    pub tx: broadcast::Sender<PanelEvent>,
}

impl PanelHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PanelEvent> {
        self.tx.subscribe()
    }
}

use peakcore_core::{History, SysSnapshot};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct MetricsStore {
    pub snapshot: SysSnapshot,
    pub history: History,
}

pub type SharedMetrics = Arc<Mutex<MetricsStore>>;

pub fn new_shared_metrics() -> SharedMetrics {
    Arc::new(Mutex::new(MetricsStore::default()))
}

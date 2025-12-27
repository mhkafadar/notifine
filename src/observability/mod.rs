pub mod alerts;
pub mod metrics;
pub mod startup;
pub mod telegram_errors;

use alerts::AlertManager;
use lazy_static::lazy_static;
use metrics::Metrics;

lazy_static! {
    pub static ref METRICS: Metrics = Metrics::new();
    pub static ref ALERTS: AlertManager = AlertManager::new();
}

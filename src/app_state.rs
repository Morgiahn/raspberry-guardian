use std::sync::Arc;
use tokio::sync::Mutex;
use crate::battery::BatteryMonitor;
use crate::state_machine::StateController;
use crate::shelly::ShellyController;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub state_controller: Arc<Mutex<StateController>>,
    pub shelly: Arc<ShellyController>,
    pub config: Arc<Config>,
    pub battery_monitor: Arc<Mutex<BatteryMonitor>>,
}
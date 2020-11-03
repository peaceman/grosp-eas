mod controller;
pub mod discovery;
mod scaler;

use serde::Deserialize;

pub use controller::NodeGroupsController;
pub use scaler::NodeGroupScaler;

#[derive(Debug, Clone, Deserialize)]
pub struct NodeGroup {
    name: String,
    config: Option<Config>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    node_bandwidth_capacity: BandwidthCapacity,
    bandwidth_thresholds: BandwidthThresholds,
    min_nodes: Option<u64>,
    max_nodes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BandwidthCapacity {
    pub tx_bps: u64,
    pub rx_bps: u64,
}

#[derive(Debug, Clone, Deserialize, Copy)]
pub struct BandwidthThresholds {
    scale_up_percent: u8,
    scale_down_percent: u8,
}

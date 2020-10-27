use act_zero::{Actor, ActorResult, Produces};
use async_trait::async_trait;
use serde::Deserialize;

mod controller;
pub mod discovery;
pub mod exploration;
pub mod stats;

pub use controller::NodeController;

#[derive(Debug)]
pub struct NodeStats {
    pub tx_bps: u64,
    pub rx_bps: u64,
}

#[async_trait]
pub trait NodeStatsObserver: Actor {
    async fn observe_node_stats(&mut self, _stats: NodeStats) -> ActorResult<()> {
        Produces::ok(())
    }
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub enum NodeDrainingCause {
    Scaling,
    RollingUpdate,
}

#[derive(Debug)]
pub enum NodeState {
    Unready,
    Ready,
    Active,
    Draining(NodeDrainingCause),
    Deprovisioned,
}

#[derive(Debug)]
pub struct NodeStateInfo {
    pub hostname: String,
    pub state: NodeState,
}

#[async_trait]
pub trait NodeStateObserver: Actor {
    async fn observe_node_state(&mut self, state_info: NodeStateInfo) {
        ()
    }
}

use act_zero::{Actor, ActorResult, Produces};
use async_trait::async_trait;

mod controller;
pub mod discovery;
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

#[derive(Debug, Copy, Clone)]
pub enum NodeDrainingCause {
    Scaling,
    RollingUpdate,
}

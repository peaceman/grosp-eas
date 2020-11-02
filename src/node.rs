use act_zero::{Actor, ActorResult, Produces};
use async_trait::async_trait;
use serde::Deserialize;
use std::fmt;

mod controller;
pub mod discovery;
pub mod exploration;
pub mod stats;

pub use controller::NodeController;

#[derive(Debug, Clone)]
pub struct Node {
    pub hostname: String,
    pub group: String,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node {}@{}", self.hostname, self.group)
    }
}

#[derive(Debug)]
pub struct NodeStats {
    pub tx_bps: u64,
    pub rx_bps: u64,
}

#[derive(Debug)]
pub struct NodeStatsInfo {
    pub hostname: String,
    pub stats: NodeStats,
}

#[async_trait]
pub trait NodeStatsObserver: Actor {
    async fn observe_node_stats(&mut self, stats_info: NodeStatsInfo) {}
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub enum NodeDrainingCause {
    Scaling,
    RollingUpdate,
    Termination,
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
    async fn observe_node_state(&mut self, state_info: NodeStateInfo) {}
}

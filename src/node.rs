use act_zero::Actor;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

mod controller;
pub mod discovery;
pub mod exploration;
mod hostname;
pub mod stats;

use crate::node::discovery::NodeDiscoveryState;
pub use controller::NodeController;
pub use controller::Providers as NodeControllerProviders;
pub use hostname::HostnameGenerator;

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

#[derive(Debug, Deserialize)]
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
    async fn observe_node_stats(&mut self, stats_info: NodeStatsInfo);
}

#[derive(Debug, Copy, Clone, Deserialize, PartialEq, Serialize)]
pub enum NodeDrainingCause {
    Scaling,
    RollingUpdate,
    Termination,
}

impl fmt::Display for NodeDrainingCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                NodeDrainingCause::Scaling => "scaling",
                NodeDrainingCause::RollingUpdate => "rollingupdate",
                NodeDrainingCause::Termination => "termination",
            }
        )
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum NodeState {
    Unready,
    Ready,
    Active,
    Draining(NodeDrainingCause),
    Deprovisioned,
}

impl NodeState {
    pub fn is_active(&self) -> bool {
        matches!(self, NodeState::Active)
    }

    pub fn is_draining(&self, cause: NodeDrainingCause) -> bool {
        matches!(self, NodeState::Draining(ic) if *ic == cause)
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, NodeState::Ready)
    }
}

impl From<NodeDiscoveryState> for NodeState {
    fn from(s: NodeDiscoveryState) -> Self {
        match s {
            NodeDiscoveryState::Ready => NodeState::Ready,
            NodeDiscoveryState::Active => NodeState::Active,
            NodeDiscoveryState::Draining(cause) => NodeState::Draining(cause),
        }
    }
}

#[derive(Debug)]
pub struct NodeStateInfo {
    pub hostname: String,
    pub state: NodeState,
}

#[async_trait]
pub trait NodeStateObserver: Actor {
    async fn observe_node_state(&mut self, state_info: NodeStateInfo);
}

mod file;

use crate::node::NodeDrainingCause;
use act_zero::{Actor, ActorResult};
use async_trait::async_trait;
use serde::Deserialize;

pub use file::FileNodeDiscovery;

#[async_trait]
pub trait NodeDiscoveryProvider: Actor {
    async fn update_state(
        &mut self,
        hostname: String,
        state: NodeDiscoveryState,
    ) -> ActorResult<()>;
}

#[async_trait]
pub trait NodeDiscoveryObserver: Actor {
    async fn observe_node_discovery(&mut self, data: NodeDiscoveryData);
}

#[derive(Debug, Deserialize)]
pub struct NodeDiscoveryData {
    pub hostname: String,
    pub group: String,
    pub state: NodeDiscoveryState,
}

#[derive(Debug, Deserialize)]
pub enum NodeDiscoveryState {
    Ready,
    Active,
    Draining(NodeDrainingCause),
}

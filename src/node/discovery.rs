use crate::node::NodeDrainingCause;
use act_zero::{Actor, ActorResult};
use async_trait::async_trait;

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

#[derive(Debug)]
pub struct NodeDiscoveryData {
    pub hostname: String,
    pub state: NodeDiscoveryState,
}

#[derive(Debug)]
pub enum NodeDiscoveryState {
    Ready,
    Active,
    Draining(NodeDrainingCause),
}

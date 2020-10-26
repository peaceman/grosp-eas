use crate::cloud_provider::CloudNodeInfo;
use act_zero::Actor;
use async_trait::async_trait;

#[async_trait]
pub trait NodeExplorationObserver: Actor {
    async fn observe_node_exploration(&mut self, node_info: CloudNodeInfo);
}

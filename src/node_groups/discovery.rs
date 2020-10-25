mod file;

use crate::node_groups::NodeGroup;
use act_zero::Actor;
use async_trait::async_trait;
pub use file::FileNodeGroupDiscovery;

#[async_trait]
pub trait NodeGroupDiscoveryObserver: Actor {
    async fn observe_node_group_discovery(&mut self, node_group: NodeGroup);
}

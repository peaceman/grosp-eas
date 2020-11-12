use super::NodeDiscoveryProvider;
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryState};
use act_zero::{Actor, ActorResult, Produces};
use async_trait::async_trait;
use tracing::info;

pub struct MockNodeDiscovery;

impl Actor for MockNodeDiscovery {}

#[async_trait]
impl NodeDiscoveryProvider for MockNodeDiscovery {
    async fn update_state(
        &mut self,
        hostname: String,
        state: NodeDiscoveryState,
    ) -> ActorResult<()> {
        info!("Updating state of node {} {:?}", hostname, state);

        Produces::ok(())
    }

    async fn discover_nodes(&mut self) -> ActorResult<Vec<NodeDiscoveryData>> {
        Produces::ok(vec![])
    }
}

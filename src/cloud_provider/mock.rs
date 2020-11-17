use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::node::discovery::NodeDiscoveryState;
use act_zero::{Actor, ActorResult, Addr, Produces};
use async_trait::async_trait;
use tracing::info;

struct MockCloudProvider;

#[async_trait]
impl Actor for MockCloudProvider {
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started MockCloudProvider");

        Produces::ok(())
    }
}

#[async_trait]
impl CloudProvider for MockCloudProvider {
    async fn get_node_info(&mut self, _hostname: String) -> ActorResult<Option<CloudNodeInfo>> {
        Produces::ok(None)
    }

    async fn create_node(
        &mut self,
        _hostname: String,
        _target_state: NodeDiscoveryState,
    ) -> ActorResult<CloudNodeInfo> {
        unimplemented!()
    }

    async fn delete_node(&mut self, _node_info: CloudNodeInfo) -> ActorResult<()> {
        Produces::ok(())
    }

    async fn get_nodes(&mut self) -> ActorResult<Vec<CloudNodeInfo>> {
        Produces::ok(vec![])
    }
}

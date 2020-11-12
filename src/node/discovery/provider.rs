mod file;
mod mock;

use crate::config;
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryState};
use crate::AppConfig;
use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{upcast, Actor, ActorResult, Addr};
use async_trait::async_trait;

#[async_trait]
pub trait NodeDiscoveryProvider: Actor {
    async fn update_state(
        &mut self,
        hostname: String,
        state: NodeDiscoveryState,
    ) -> ActorResult<()>;

    async fn discover_nodes(&mut self) -> ActorResult<Vec<NodeDiscoveryData>>;
}

pub fn build_from_config(config: AppConfig) -> anyhow::Result<Addr<dyn NodeDiscoveryProvider>> {
    Ok(match &config.node_discovery_provider {
        config::NodeDiscoveryProvider::Mock => upcast!(spawn_actor(mock::MockNodeDiscovery)),
        config::NodeDiscoveryProvider::File { path } => {
            upcast!(spawn_actor(file::FileNodeDiscovery::new(path)))
        }
    })
}

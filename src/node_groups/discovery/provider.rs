use crate::config;
use crate::node_groups::NodeGroup;
use crate::AppConfig;
use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{upcast, Actor, ActorResult, Addr};
use async_trait::async_trait;

pub mod consul;
pub mod file;

#[async_trait]
pub trait NodeGroupDiscoveryProvider: Actor {
    async fn discover_node_groups(&mut self) -> ActorResult<Vec<NodeGroup>>;
}

pub fn build_from_config(
    config: AppConfig,
) -> anyhow::Result<Vec<Addr<dyn NodeGroupDiscoveryProvider>>> {
    let mut providers = Vec::with_capacity(config.node_group_discovery_providers.len());

    for provider_config in config.node_group_discovery_providers.iter() {
        providers.push(build_provider_from_config(&config, provider_config)?);
    }

    Ok(providers)
}

fn build_provider_from_config(
    _config: &AppConfig,
    provider_config: &config::NodeGroupDiscoveryProvider,
) -> anyhow::Result<Addr<dyn NodeGroupDiscoveryProvider>> {
    Ok(match provider_config {
        config::NodeGroupDiscoveryProvider::File { path } => {
            upcast!(spawn_actor(file::FileNodeGroupDiscovery::new(path)))
        }
        config::NodeGroupDiscoveryProvider::Consul {
            key_prefix,
            address,
        } => {
            let consul_client = crate::consul::Client::new(
                crate::consul::Config::builder()
                    .address(address.into())
                    .build()?,
            )?;

            upcast!(spawn_actor(consul::ConsulNodeGroupDiscovery::new(
                consul_client,
                key_prefix.into(),
            )))
        }
    })
}

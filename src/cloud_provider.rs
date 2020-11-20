mod file;
mod hetzner;
mod mock;

use act_zero::{Actor, ActorResult, Addr};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use crate::config;
use crate::node::discovery::NodeDiscoveryState;
use crate::AppConfig;
use act_zero::runtimes::tokio::spawn_actor;
use act_zero::upcast;
pub use file::FileCloudProvider;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CloudNodeInfo {
    pub identifier: String,
    pub hostname: String,
    pub group: String,
    pub created_at: DateTime<Utc>,
    pub ip_addresses: Vec<IpAddr>,
}

#[async_trait]
pub trait CloudProvider: Actor {
    async fn get_node_info(&mut self, hostname: String) -> ActorResult<Option<CloudNodeInfo>>;
    async fn create_node(
        &mut self,
        hostname: String,
        group: String,
        target_state: NodeDiscoveryState,
    ) -> ActorResult<CloudNodeInfo>;
    async fn delete_node(&mut self, node_info: CloudNodeInfo) -> ActorResult<()>;
    async fn get_nodes(&mut self) -> ActorResult<Vec<CloudNodeInfo>>;
}

pub fn build_from_config(config: AppConfig) -> anyhow::Result<Addr<dyn CloudProvider>> {
    Ok(match &config.cloud_provider {
        config::CloudProvider::File {
            exploration_path,
            discovery_path,
        } => upcast!(spawn_actor(FileCloudProvider::new(
            exploration_path,
            discovery_path
        ))),
    })
}

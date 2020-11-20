use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::hetzner_cloud;
use crate::hetzner_cloud::error::Error;
use crate::hetzner_cloud::servers::{Server, Servers};
use crate::node::discovery::NodeDiscoveryState;
use act_zero::{Actor, ActorResult, Addr, Produces};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::net::IpAddr;
use tracing::error;
use tracing::info;
use tracing::warn;

pub struct HetznerCloudProvider {
    client: hetzner_cloud::Client,
    config: Config,
}

#[derive(Clone, Debug)]
pub struct Config {
    group_label_name: String,
}

impl HetznerCloudProvider {
    pub fn new(client: hetzner_cloud::Client, config: Config) -> Self {
        Self { client, config }
    }
}

#[async_trait]
impl Actor for HetznerCloudProvider {
    #[tracing::instrument(name = "HetznerCloudProvider::started", skip(self, _addr))]
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");
        Produces::ok(())
    }
}

#[async_trait]
impl CloudProvider for HetznerCloudProvider {
    #[tracing::instrument(name = "HetznerCloudProvider::get_node_info", skip(self))]
    async fn get_node_info(&mut self, hostname: String) -> ActorResult<Option<CloudNodeInfo>> {
        let server = self.client.search_server(&hostname).await;

        Produces::ok(match server {
            Ok(Some(server)) => {
                match create_cloud_node_info(server, &self.config.group_label_name) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!("Failed to create cloud node info: {:?}", e);
                        None
                    }
                }
            }
            Ok(None) => None,
            Err(e) => {
                error!("Failed to get node info: {:?}", e);

                None
            }
        })
    }

    async fn create_node(
        &mut self,
        hostname: String,
        target_state: NodeDiscoveryState,
    ) -> ActorResult<CloudNodeInfo> {
        unimplemented!()
    }

    async fn delete_node(&mut self, node_info: CloudNodeInfo) -> ActorResult<()> {
        unimplemented!()
    }

    async fn get_nodes(&mut self) -> ActorResult<Vec<CloudNodeInfo>> {
        unimplemented!()
    }
}

fn create_cloud_node_info(server: Server, group_label_name: &str) -> Result<CloudNodeInfo> {
    let group = match server.labels.get(group_label_name) {
        Some(v) => v.clone(),
        None => return Err(anyhow!("Missing node group label `{}`", group_label_name)),
    };

    let ip_addresses = server.get_ip_addresses();
    let cni = CloudNodeInfo {
        identifier: server.id.to_string(),
        hostname: server.name,
        created_at: server.created,
        group,
        ip_addresses,
    };

    Ok(cni)
}

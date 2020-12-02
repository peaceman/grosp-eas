use crate::cloud_init::user_data::GenerateUserData;
use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::hetzner_cloud::error::Error;
use crate::hetzner_cloud::servers::{NewServer, Server, Servers};
use crate::node::discovery::NodeDiscoveryState;
use crate::{actor, hetzner_cloud};
use act_zero::{Actor, ActorError, ActorResult, Addr, Produces};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use http::StatusCode;
use std::collections::HashMap;
use tracing::error;
use tracing::info;
use tracing::warn;

pub struct HetznerCloudProvider<UDG: GenerateUserData> {
    client: hetzner_cloud::Client,
    config: Config,
    user_data_generator: UDG,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub group_label_name: String,
    pub server_type: String,
    pub image: String,
    pub ssh_keys: Vec<String>,
    pub location: Option<String>,
}

impl<UDG: GenerateUserData> HetznerCloudProvider<UDG> {
    pub fn new(client: hetzner_cloud::Client, config: Config, user_data_generator: UDG) -> Self {
        Self {
            client,
            config,
            user_data_generator,
        }
    }
}

#[async_trait]
impl<UDG> Actor for HetznerCloudProvider<UDG>
where
    UDG: GenerateUserData + Send + 'static,
{
    #[tracing::instrument(name = "HetznerCloudProvider::started", skip(self, _addr))]
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        actor::handle_error(error)
    }
}

#[async_trait]
impl<UDG> CloudProvider for HetznerCloudProvider<UDG>
where
    UDG: GenerateUserData + Send + 'static,
{
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

    #[tracing::instrument(name = "HetznerCloudProvider::create_node", skip(self))]
    async fn create_node(
        &mut self,
        hostname: String,
        group: String,
        target_state: NodeDiscoveryState,
    ) -> ActorResult<CloudNodeInfo> {
        let mut labels = HashMap::new();
        labels.insert(self.config.group_label_name.clone(), group.clone());

        let user_data =
            match gen_user_data(&hostname, &group, &target_state, &self.user_data_generator) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to generate user data: {:?}", e);
                    return Err(e.into());
                }
            };

        let server = NewServer {
            name: &hostname,
            server_type: &self.config.server_type,
            image: &self.config.image,
            ssh_keys: self
                .config
                .ssh_keys
                .iter()
                .map(|ssh_key| ssh_key.as_ref())
                .collect(),
            user_data: Some(&user_data),
            labels: Some(&labels),
            location: self.config.location.as_ref().map(|s| s.as_str()),
        };

        let server = match self.client.create_server(&server).await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to create server: {:?}", e);
                return Err(e.into());
            }
        };

        Produces::ok(
            match create_cloud_node_info(server, &self.config.group_label_name) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to create cloud node info: {:?}", e);
                    return Err(e.into());
                }
            },
        )
    }

    #[tracing::instrument(name = "HetznerCloudProvider::delete_node", skip(self))]
    async fn delete_node(&mut self, node_info: CloudNodeInfo) -> ActorResult<()> {
        let server_id: u64 = match node_info.identifier.parse() {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse node identifier: {:?}", e);
                return Err(e.into());
            }
        };

        match self.client.delete_server(server_id).await {
            Ok(_) => Produces::ok(()),
            Err(e) => match e {
                Error::BadResponse { status, .. } if status == StatusCode::NOT_FOUND => {
                    Produces::ok(())
                }
                _ => {
                    error!("Failed to delete server: {:?}", e);
                    Err(e.into())
                }
            },
        }
    }

    #[tracing::instrument(name = "HetznerCloudProvider::get_nodes", skip(self))]
    async fn get_nodes(&mut self) -> ActorResult<Vec<CloudNodeInfo>> {
        let selector = &self.config.group_label_name;
        let servers = match self.client.get_all_servers(Some(selector)).await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to fetch nodes: {:?}", e);

                return Produces::ok(vec![]);
            }
        };

        let nodes = servers
            .into_iter()
            .filter_map(
                |s| match create_cloud_node_info(s, &self.config.group_label_name) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!("Failed to create cloud node info: {:?}", e);
                        None
                    }
                },
            )
            .collect();

        Produces::ok(nodes)
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

fn gen_user_data<UDG: GenerateUserData>(
    hostname: &str,
    group: &str,
    target_state: &NodeDiscoveryState,
    generator: &UDG,
) -> Result<String> {
    let mut user_data: Vec<u8> = vec![];
    let target_state = target_state.to_string();

    generator.generate_user_data(&hostname, group, target_state.as_str(), &mut user_data)?;

    Ok(String::from_utf8(user_data)?)
}

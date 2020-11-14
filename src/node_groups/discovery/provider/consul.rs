use crate::consul::kv::KV;
use crate::consul::Client;
use crate::node_groups::discovery::provider::NodeGroupDiscoveryProvider;
use crate::node_groups::NodeGroup;
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use anyhow::anyhow;
use anyhow::Context;
use async_trait::async_trait;
use tracing::{info, warn, Instrument};

pub struct ConsulNodeGroupDiscovery {
    consul_client: Client,
    key_prefix: String,
}

impl ConsulNodeGroupDiscovery {
    pub fn new(consul_client: Client, key_prefix: String) -> Self {
        Self {
            consul_client,
            key_prefix,
        }
    }
}

#[async_trait]
impl Actor for ConsulNodeGroupDiscovery {
    #[tracing::instrument(name = "ConsulNodeGroupDiscovery::started", skip(self, addr))]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

        Produces::ok(())
    }
}

#[async_trait]
impl NodeGroupDiscoveryProvider for ConsulNodeGroupDiscovery {
    #[tracing::instrument(name = "ConsulNodeGroupDiscovery::discover_node_groups", skip(self))]
    async fn discover_node_groups(&mut self) -> ActorResult<Vec<NodeGroup>> {
        let (kv_pairs, _meta) = self.consul_client.list(&self.key_prefix, None).await?;

        let node_groups: Vec<NodeGroup> = kv_pairs
            .iter()
            .filter_map(|kv_pair| match base64::decode(&kv_pair.Value) {
                Ok(v) => Some((&kv_pair.Key, v)),
                Err(e) => {
                    warn!(
                        error = format!("{:?}", e).as_str(),
                        "Failed to decode base64 node group data"
                    );
                    None
                }
            })
            .filter_map(|(key, data)| {
                match serde_yaml::from_reader::<&[u8], NodeGroup>(data.as_ref()) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!(
                            error = format!("{:?}", e).as_str(),
                            key = key.as_str(),
                            "Failed to parse node group yaml"
                        );

                        None
                    }
                }
            })
            .collect();

        Produces::ok(node_groups)
    }
}

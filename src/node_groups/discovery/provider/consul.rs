use crate::actor;
use crate::node_groups::discovery::provider::NodeGroupDiscoveryProvider;
use crate::node_groups::NodeGroup;
use act_zero::{Actor, ActorError, ActorResult, Addr, Produces};
use async_trait::async_trait;
use consul_api_client::kv::KV;
use tracing::{info, warn};

pub struct ConsulNodeGroupDiscovery {
    consul_client: consul_api_client::Client,
    key_prefix: String,
}

impl ConsulNodeGroupDiscovery {
    pub fn new(consul_client: consul_api_client::Client, key_prefix: String) -> Self {
        Self {
            consul_client,
            key_prefix,
        }
    }
}

#[async_trait]
impl Actor for ConsulNodeGroupDiscovery {
    #[tracing::instrument(name = "ConsulNodeGroupDiscovery::started", skip(self, _addr))]
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
impl NodeGroupDiscoveryProvider for ConsulNodeGroupDiscovery {
    #[tracing::instrument(name = "ConsulNodeGroupDiscovery::discover_node_groups", skip(self))]
    async fn discover_node_groups(&mut self) -> ActorResult<Vec<NodeGroup>> {
        let (kv_pairs, _meta) = self
            .consul_client
            .list(&self.key_prefix, None)
            .await
            .map_err(anyhow::Error::new)
            .map_err(actor::Error::from)?;

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

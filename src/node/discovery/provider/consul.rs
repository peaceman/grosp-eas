use crate::consul::catalog::Catalog;
use crate::consul::health::{Health, ServiceEntry};
use crate::consul::Client as ConsulClient;
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryProvider, NodeDiscoveryState};
use crate::node::NodeDrainingCause::{RollingUpdate, Scaling, Termination};
use crate::node::NodeState;
use act_zero::{Actor, ActorResult, Addr, Produces};
use async_trait::async_trait;
use opentelemetry::api::trace::span::SpanKind::Producer;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

pub struct ConsulNodeDiscovery {
    consul: ConsulClient,
    service_name: String,
}

impl ConsulNodeDiscovery {
    pub fn new(consul: ConsulClient, service_name: String) -> Self {
        Self {
            consul,
            service_name,
        }
    }
}

#[async_trait]
impl Actor for ConsulNodeDiscovery {
    #[tracing::instrument(
        name = "ConsulNodeDiscovery::started"
        skip(self, addr),
    )]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

        Produces::ok(())
    }
}

#[async_trait]
impl NodeDiscoveryProvider for ConsulNodeDiscovery {
    async fn update_state(
        &mut self,
        hostname: String,
        state: NodeDiscoveryState,
    ) -> ActorResult<()> {
        unimplemented!()
    }

    #[tracing::instrument(name = "ConsulNodeDiscovery::discover_nodes", skip(self))]
    async fn discover_nodes(&mut self) -> ActorResult<Vec<NodeDiscoveryData>> {
        let (services, meta) = self
            .consul
            .service(&self.service_name, None, true, None, None)
            .await?;

        let ndd = services
            .into_iter()
            .filter_map(|se| -> Option<NodeDiscoveryData> {
                let node = se.Node.Node.clone();

                se.try_into()
                    .map_err(|e: String| {
                        warn!(
                            error = e.as_str(),
                            node = node.as_str(),
                            "Failed to convert ServiceEntry into NodeDiscoveryData"
                        );

                        e
                    })
                    .ok()
            })
            .collect();

        Produces::ok(ndd)
    }
}

impl TryFrom<ServiceEntry> for NodeDiscoveryData {
    type Error = String;

    fn try_from(s: ServiceEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            hostname: s.Node.Node,
            group: s
                .Service
                .Meta
                .get("node_group")
                .ok_or_else(|| "Missing node group in service meta".to_owned())?
                .clone(),
            state: s
                .Service
                .Tags
                .ok_or_else(|| "Missing service tags".to_owned())
                .and_then(|tags| {
                    parse_node_state_from_tags(&tags)
                        .ok_or_else(|| format!("Failed to parse node state from tags {:?}", &tags))
                })?,
        })
    }
}

fn parse_node_state_from_tags(tags: &Vec<String>) -> Option<NodeDiscoveryState> {
    tags.iter().find_map(|tag| {
        if !tag.starts_with("state=") {
            None
        } else {
            let parts: Vec<&str> = tag.splitn(2, "=").collect();

            parts
                .get(1)
                .and_then(|state| -> Option<NodeDiscoveryState> { state.parse().ok() })
        }
    })
}

impl FromStr for NodeDiscoveryState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "draining-scaling" => NodeDiscoveryState::Draining(Scaling),
            "draining-rollingupdate" => NodeDiscoveryState::Draining(RollingUpdate),
            "draining-termination" => NodeDiscoveryState::Draining(Termination),
            "active" => NodeDiscoveryState::Active,
            "ready" => NodeDiscoveryState::Ready,
            _ => return Err(format!("Unknown node discovery state: {}", s)),
        })
    }
}

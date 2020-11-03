use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryObserver, NodeDiscoveryProvider};
use crate::node::exploration::NodeExplorationObserver;
use crate::node::stats::NodeStatsStreamFactory;
use crate::node::HostnameGenerator;
use crate::node_groups::scaler::NodeGroupScaler;
use crate::node_groups::NodeGroup;
use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{send, Addr, AddrLike};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Debug)]
pub enum Event {
    Initialize,
    Discovered { node_group: NodeGroup },
    Discard,
    DiscoveredNode { discovery_data: NodeDiscoveryData },
    ExploredNode { node_info: CloudNodeInfo },
}

#[async_trait]
trait Handler {
    async fn handle(self, event: Option<Event>) -> NodeGroupMachine;
}

#[derive(Debug)]
pub struct Data<S> {
    shared: Shared,
    state: S,
}

#[derive(Debug)]
pub struct Shared {
    node_group: NodeGroup,
    node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    hostname_generator: Arc<dyn HostnameGenerator>,
    discovery_timeout: Duration,
}

#[derive(Debug)]
pub struct Initializing;

#[async_trait]
impl Handler for Data<Initializing> {
    #[tracing::instrument(
        name = "NodeGroupMachine::Initializing::handle"
        skip(self),
        fields(group = self.shared.node_group.name.as_str())
    )]
    async fn handle(self, event: Option<Event>) -> NodeGroupMachine {
        match event {
            Some(Event::Initialize) => {
                let scaler = spawn_actor(NodeGroupScaler::new(
                    self.shared.node_group.clone(),
                    self.shared.node_discovery_provider.clone(),
                    self.shared.cloud_provider.clone(),
                    self.shared.dns_provider.clone(),
                    self.shared.node_stats_stream_factory.clone(),
                    Arc::clone(&self.shared.hostname_generator),
                ));

                NodeGroupMachine::Running(Data {
                    shared: self.shared,
                    state: Running {
                        scaler,
                        last_discovery: Instant::now(),
                    },
                })
            }
            _ => NodeGroupMachine::Initializing(self),
        }
    }
}

#[derive(Debug)]
pub struct Running {
    scaler: Addr<NodeGroupScaler>,
    last_discovery: Instant,
}

#[async_trait]
impl Handler for Data<Running> {
    #[tracing::instrument(
        name = "NodeGroupMachine::Running::handle"
        skip(self),
        fields(group = self.shared.node_group.name.as_str())
    )]
    async fn handle(self, event: Option<Event>) -> NodeGroupMachine {
        match event {
            Some(Event::Discovered { node_group }) => {
                send!(self
                    .state
                    .scaler
                    .update_node_group_config(node_group.config));

                NodeGroupMachine::Running(Data {
                    shared: self.shared,
                    state: Running {
                        last_discovery: Instant::now(),
                        ..self.state
                    },
                })
            }
            Some(Event::Discard) => NodeGroupMachine::Discarding(Data {
                shared: self.shared,
                state: Discarding::new(self.state.scaler),
            }),
            Some(Event::DiscoveredNode { discovery_data }) => {
                send!(self.state.scaler.observe_node_discovery(discovery_data));
                NodeGroupMachine::Running(self)
            }
            Some(Event::ExploredNode { node_info }) => {
                send!(self.state.scaler.observe_node_exploration(node_info));
                NodeGroupMachine::Running(self)
            }
            None => self.check_last_discovery().await,
            _ => NodeGroupMachine::Running(self),
        }
    }
}

impl Data<Running> {
    async fn check_last_discovery(self) -> NodeGroupMachine {
        let should_discard = Instant::now().duration_since(self.state.last_discovery)
            > self.shared.discovery_timeout;

        if should_discard {
            info!(
                timeout_secs = self.shared.discovery_timeout.as_secs(),
                "NodeGroup reached discovery timeout",
            );
            self.handle(Some(Event::Discard)).await
        } else {
            NodeGroupMachine::Running(self)
        }
    }
}

#[derive(Debug)]
pub struct Discarding {
    scaler: Addr<NodeGroupScaler>,
}

impl Discarding {
    fn new(scaler: Addr<NodeGroupScaler>) -> Self {
        Discarding { scaler }
    }
}

#[async_trait]
impl Handler for Data<Discarding> {
    #[tracing::instrument(
        name = "NodeGroupMachine::Discarding::handle"
        skip(self),
        fields(group = self.shared.node_group.name.as_str())
    )]
    async fn handle(mut self, _event: Option<Event>) -> NodeGroupMachine {
        info!("Trigger NodeGroupScaler termination");
        send!(self.state.scaler.terminate());

        let scaler_is_terminated = tokio::select! {
            _ = self.state.scaler.termination() => {
                true
            }
            _ = async { true } => {
                false
            }
        };

        if scaler_is_terminated {
            NodeGroupMachine::Discarded(Data {
                shared: self.shared,
                state: Discarded,
            })
        } else {
            NodeGroupMachine::Discarding(self)
        }
    }
}

#[derive(Debug)]
pub struct Discarded;

#[derive(Debug)]
pub enum NodeGroupMachine {
    Initializing(Data<Initializing>),
    Running(Data<Running>),
    Discarding(Data<Discarding>),
    Discarded(Data<Discarded>),
}

impl NodeGroupMachine {
    pub fn new(
        node_group: NodeGroup,
        node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
        hostname_generator: Arc<dyn HostnameGenerator>,
        discovery_timeout: Duration,
    ) -> Self {
        Self::Initializing(Data {
            shared: Shared {
                node_group,
                node_discovery_provider,
                cloud_provider,
                dns_provider,
                node_stats_stream_factory,
                hostname_generator,
                discovery_timeout,
            },
            state: Initializing,
        })
    }

    pub async fn handle(self, event: Option<Event>) -> Self {
        match self {
            Self::Initializing(m) => m.handle(event).await,
            Self::Running(m) => m.handle(event).await,
            Self::Discarding(m) => m.handle(event).await,
            Self::Discarded(_) => self,
        }
    }
}

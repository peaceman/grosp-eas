mod active;
mod deprovisioned;
mod deprovisioning;
mod discovering;
mod draining;
mod exploring;
mod initializing;
mod provisioning;
mod ready;

use super::Config;
use super::StatsStreamer;
use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryProvider, NodeDiscoveryState};
use crate::node::stats::NodeStatsStreamFactory;
use crate::node::{
    Node, NodeDrainingCause, NodeState, NodeStateInfo, NodeStateObserver, NodeStatsObserver,
};
use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{call, send, Addr, WeakAddr};
use async_trait::async_trait;
use std::convert::AsRef;
use std::fmt;
use std::string::ToString;
use std::time::{Duration, Instant};
use strum_macros::AsRefStr;
use tracing::{error, info, trace};

#[derive(strum_macros::Display, Debug)]
pub enum NodeMachine {
    Initializing(Data<Initializing>),
    Provisioning(Data<Provisioning>),
    Exploring(Data<Exploring>),
    Discovering(Data<Discovering>),
    Ready(Data<Ready>),
    Active(Data<Active>),
    Draining(Data<Draining>),
    Deprovisioning(Data<Deprovisioning>),
    Deprovisioned(Data<Deprovisioned>),
}

#[derive(Debug)]
pub enum NodeMachineEvent {
    ProvisionNode { target_state: NodeDiscoveryState },
    DiscoveredNode { discovery_data: NodeDiscoveryData },
    ExploredNode { node_info: CloudNodeInfo },
    ActivateNode,
    DeprovisionNode { cause: NodeDrainingCause },
}

#[async_trait]
trait Handler {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine;
}

pub trait MachineState {}

#[derive(Debug)]
pub struct Data<S: MachineState> {
    shared: Shared,
    state: S,
}

#[derive(Debug)]
pub struct Shared {
    node: Node,
    node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_stats_observer: WeakAddr<dyn NodeStatsObserver>,
    node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    config: Config,
}

#[derive(Debug)]
pub struct Initializing {}

#[derive(Debug)]
pub struct Provisioning {
    node_info: Option<CloudNodeInfo>,
    entered_state_at: Instant,
    created_dns_records: bool,
    target_state: NodeDiscoveryState,
}

impl Provisioning {
    fn new(target_state: NodeDiscoveryState) -> Self {
        Self {
            node_info: None,
            entered_state_at: Instant::now(),
            created_dns_records: false,
            target_state,
        }
    }
}

#[derive(Debug)]
pub struct Exploring {
    pub discovery_data: NodeDiscoveryData,
}

#[derive(Debug)]
pub struct Discovering {
    node_info: CloudNodeInfo,
    entered_state_at: Instant,
}

impl Discovering {
    fn new(node_info: CloudNodeInfo) -> Self {
        Self {
            node_info,
            entered_state_at: Instant::now(),
        }
    }
}

#[derive(Debug)]
pub struct Ready {
    node_info: CloudNodeInfo,
    entered_state_at: Instant,
    last_discovered_at: Option<Instant>,
    stats_streamer: Option<Addr<StatsStreamer>>,
}

impl Ready {
    fn new(node_info: CloudNodeInfo, stats_streamer: Option<Addr<StatsStreamer>>) -> Self {
        Self {
            node_info,
            entered_state_at: Instant::now(),
            last_discovered_at: None,
            stats_streamer,
        }
    }
}

#[derive(Debug)]
pub struct Active {
    node_info: CloudNodeInfo,
    marked_as_active: bool,
    entered_state_at: Instant,
    last_discovered_at: Option<Instant>,
    stats_streamer: Option<Addr<StatsStreamer>>,
}

impl Active {
    fn new(node_info: CloudNodeInfo, stats_streamer: Option<Addr<StatsStreamer>>) -> Self {
        Self {
            node_info,
            marked_as_active: false,
            entered_state_at: Instant::now(),
            last_discovered_at: None,
            stats_streamer,
        }
    }

    fn new_marked(node_info: CloudNodeInfo, stats_streamer: Option<Addr<StatsStreamer>>) -> Self {
        Self {
            node_info,
            marked_as_active: true,
            entered_state_at: Instant::now(),
            last_discovered_at: None,
            stats_streamer,
        }
    }
}

#[derive(Debug)]
pub struct Draining {
    node_info: CloudNodeInfo,
    cause: NodeDrainingCause,
    marked_as_draining: bool,
    entered_state_at: Instant,
    stats_streamer: Option<Addr<StatsStreamer>>,
}

impl Draining {
    fn new(
        node_info: CloudNodeInfo,
        cause: NodeDrainingCause,
        stats_streamer: Option<Addr<StatsStreamer>>,
    ) -> Self {
        Self {
            node_info,
            cause,
            marked_as_draining: false,
            entered_state_at: Instant::now(),
            stats_streamer,
        }
    }

    fn new_marked(
        node_info: CloudNodeInfo,
        cause: NodeDrainingCause,
        stats_streamer: Option<Addr<StatsStreamer>>,
    ) -> Self {
        Self {
            node_info,
            cause,
            marked_as_draining: true,
            entered_state_at: Instant::now(),
            stats_streamer,
        }
    }
}

#[derive(Debug)]
pub struct Deprovisioning {
    node_info: Option<CloudNodeInfo>,
    deleted_node: bool,
    deleted_dns_records: bool,
}

impl Deprovisioning {
    fn new(node_info: Option<CloudNodeInfo>) -> Self {
        Self {
            node_info,
            deleted_node: false,
            deleted_dns_records: false,
        }
    }
}

#[derive(Debug)]
pub struct Deprovisioned;

impl NodeMachine {
    pub fn new(
        node: Node,
        node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        node_stats_observer: WeakAddr<dyn NodeStatsObserver>,
        node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
        config: Config,
    ) -> Self {
        Self::Initializing(Data {
            state: Initializing {},
            shared: Shared {
                node,
                node_discovery_provider,
                cloud_provider,
                dns_provider,
                node_stats_observer,
                node_stats_stream_factory,
                config,
            },
        })
    }

    pub async fn handle(
        self,
        event: Option<NodeMachineEvent>,
        node_state_observer: WeakAddr<dyn NodeStateObserver>,
    ) -> Self {
        let handler = NodeMachineProgressHandler::new(node_state_observer);

        let old_state = self.to_string();

        let result = match self {
            Self::Initializing(m) => handler.progress(m, event).await,
            Self::Provisioning(m) => handler.progress(m, event).await,
            Self::Exploring(m) => handler.progress(m, event).await,
            Self::Discovering(m) => handler.progress(m, event).await,
            Self::Ready(m) => handler.progress(m, event).await,
            Self::Active(m) => handler.progress(m, event).await,
            Self::Draining(m) => handler.progress(m, event).await,
            Self::Deprovisioning(m) => handler.progress(m, event).await,
            Self::Deprovisioned(m) => handler.progress(m, event).await,
        };

        let new_state = result.to_string();
        if old_state != new_state {
            info!(%old_state, %new_state, "Transition");
        }

        result
    }

    async fn handle_data<T: Handler>(
        data: T,
        event: Option<NodeMachineEvent>,
        node_state_observer: WeakAddr<dyn NodeStateObserver>,
    ) -> Self {
        let new_state = data.handle(event).await;
        new_state.publish_node_state(&node_state_observer);
        new_state
    }

    fn publish_node_state(&self, node_state_observer: &WeakAddr<dyn NodeStateObserver>) {
        let node_state_info = match self {
            NodeMachine::Initializing(Data {
                shared: Shared { node, .. },
                ..
            })
            | NodeMachine::Discovering(Data {
                shared: Shared { node, .. },
                ..
            })
            | NodeMachine::Exploring(Data {
                shared: Shared { node, .. },
                ..
            })
            | NodeMachine::Provisioning(Data {
                shared: Shared { node, .. },
                ..
            })
            | NodeMachine::Deprovisioning(Data {
                shared: Shared { node, .. },
                ..
            }) => NodeStateInfo {
                state: NodeState::Unready,
                hostname: node.hostname.clone(),
            },
            NodeMachine::Ready(Data {
                shared: Shared { node, .. },
                ..
            }) => NodeStateInfo {
                state: NodeState::Ready,
                hostname: node.hostname.clone(),
            },
            NodeMachine::Active(Data {
                shared: Shared { node, .. },
                ..
            }) => NodeStateInfo {
                state: NodeState::Active,
                hostname: node.hostname.clone(),
            },
            NodeMachine::Draining(Data {
                shared: Shared { node, .. },
                state: Draining { cause, .. },
                ..
            }) => NodeStateInfo {
                state: NodeState::Draining(cause.clone()),
                hostname: node.hostname.clone(),
            },
            NodeMachine::Deprovisioned(Data {
                shared: Shared { node, .. },
                ..
            }) => NodeStateInfo {
                state: NodeState::Deprovisioned,
                hostname: node.hostname.clone(),
            },
        };

        send!(node_state_observer.observe_node_state(node_state_info));
    }
}

struct NodeMachineProgressHandler {
    node_state_observer: WeakAddr<dyn NodeStateObserver>,
}

impl NodeMachineProgressHandler {
    fn new(node_state_observer: WeakAddr<dyn NodeStateObserver>) -> Self {
        Self {
            node_state_observer,
        }
    }

    async fn progress<T: Handler>(
        &self,
        node_machine_data: T,
        event: Option<NodeMachineEvent>,
    ) -> NodeMachine {
        let node_machine = node_machine_data.handle(event).await;
        node_machine.publish_node_state(&self.node_state_observer);
        node_machine
    }
}

fn start_stats_streamer(shared: &Shared) -> Addr<StatsStreamer> {
    info!("Start stats streamer actor");

    spawn_actor(StatsStreamer::new(
        shared.node.hostname.clone(),
        shared.node_stats_observer.clone(),
        shared.node_stats_stream_factory.clone(),
    ))
}

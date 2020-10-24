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
use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::NodeDrainingCause;
use crate::node_discovery::{NodeDiscoveryData, NodeDiscoveryProvider};
use act_zero::{call, Addr};
use async_trait::async_trait;
use log::{error, info};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum NodeState {
    Unready,
    Active,
    Draining(NodeDrainingCause),
}

#[derive(Debug)]
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

pub enum NodeMachineEvent {
    ProvisionNode,
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
    hostname: String,
    node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    config: Config,
}

#[derive(Debug)]
pub struct Initializing {}

#[derive(Debug)]
pub struct Provisioning {
    node_info: Option<CloudNodeInfo>,
    entered_state_at: Instant,
    created_dns_records: bool,
}

impl Provisioning {
    fn new() -> Self {
        Self {
            node_info: None,
            entered_state_at: Instant::now(),
            created_dns_records: false,
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
}

impl Ready {
    fn new(node_info: CloudNodeInfo) -> Self {
        Self {
            node_info,
            entered_state_at: Instant::now(),
            last_discovered_at: None,
        }
    }
}

#[derive(Debug)]
pub struct Active {
    node_info: CloudNodeInfo,
    marked_as_active: bool,
}

impl Active {
    fn new(node_info: CloudNodeInfo) -> Self {
        Self {
            node_info,
            marked_as_active: false,
        }
    }
}

#[derive(Debug)]
pub struct Draining {
    node_info: CloudNodeInfo,
    cause: NodeDrainingCause,
    marked_as_draining: bool,
    entered_state_at: Instant,
}

impl Draining {
    fn new(node_info: CloudNodeInfo, cause: NodeDrainingCause) -> Self {
        Self {
            node_info,
            cause,
            marked_as_draining: false,
            entered_state_at: Instant::now(),
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
        hostname: String,
        node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        config: Config,
    ) -> Self {
        Self::Initializing(Data {
            state: Initializing {},
            shared: Shared {
                hostname,
                node_discovery_provider,
                cloud_provider,
                dns_provider,
                config,
            },
        })
    }

    pub async fn handle(self, event: Option<NodeMachineEvent>) -> Self {
        match self {
            Self::Initializing(m) => m.handle(event).await,
            Self::Provisioning(m) => m.handle(event).await,
            Self::Exploring(m) => m.handle(event).await,
            Self::Discovering(m) => m.handle(event).await,
            Self::Ready(m) => m.handle(event).await,
            Self::Active(m) => m.handle(event).await,
            Self::Draining(m) => m.handle(event).await,
            Self::Deprovisioning(m) => m.handle(event).await,
            Self::Deprovisioned(_) => self,
        }
    }
}

mod active;
mod deprovisioned;
mod deprovisioning;
mod draining;
mod exploring;
mod initializing;
mod provisioning;
mod ready;

use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::{NodeDiscoveryData, NodeDiscoveryState};
use act_zero::{call, Addr};
use async_trait::async_trait;
use log::{error, info};
use std::time::{Duration, Instant};

pub enum NodeDrainingCause {
    Scaling,
    RollingUpdate,
}

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
    Ready(Data<Ready>),
    Active(Data<Active>),
    Draining(Data<Draining>),
    Deprovisioning(Data<Deprovisioning>),
    Deprovisioned(Data<Deprovisioned>),
}

pub enum NodeMachineEvent {
    ProvisionNode {
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        state_timeout: Duration,
    },
    DiscoveredNode {
        discovery_data: NodeDiscoveryData,
        cloud_provider: Addr<dyn CloudProvider>,
    },
}

#[async_trait]
trait Handler {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine;
}

pub trait MachineState {}

#[derive(Debug)]
pub struct Data<S: MachineState> {
    hostname: String,
    state: S,
}

#[derive(Debug)]
pub struct Initializing {}

#[derive(Debug)]
pub struct Provisioning {
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_info: Option<CloudNodeInfo>,
    entered_state_at: Instant,
    state_timeout: Duration,
    created_dns_records: bool,
}

#[derive(Debug)]
pub struct Exploring {
    pub discovery_data: NodeDiscoveryData,
    cloud_provider: Addr<dyn CloudProvider>,
}

#[derive(Debug)]
pub struct Ready {
    node_info: CloudNodeInfo,
}

#[derive(Debug)]
pub struct Active {
    node_info: CloudNodeInfo,
}

#[derive(Debug)]
pub struct Draining {
    node_info: CloudNodeInfo,
}

#[derive(Debug)]
pub struct Deprovisioning {
    node_info: Option<CloudNodeInfo>,
}

#[derive(Debug)]
pub struct Deprovisioned {}

impl NodeMachine {
    pub fn new(hostname: String) -> Self {
        Self::Initializing(Data {
            state: Initializing {},
            hostname,
        })
    }

    pub async fn handle(self, event: Option<NodeMachineEvent>) -> Self {
        match self {
            Self::Initializing(m) => m.handle(event).await,
            Self::Provisioning(m) => m.handle(event).await,
            Self::Exploring(m) => m.handle(event).await,
            Self::Ready(m) => m.handle(event).await,
            Self::Active(m) => m.handle(event).await,
            Self::Draining(m) => m.handle(event).await,
            Self::Deprovisioning(m) => m.handle(event).await,
            Self::Deprovisioned(_) => self,
        }
    }
}

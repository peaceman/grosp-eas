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

impl MachineState for Initializing {}

#[async_trait]
impl Handler for Data<Initializing> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::ProvisionNode {
                cloud_provider,
                dns_provider,
                state_timeout,
            }) => NodeMachine::Provisioning(Data {
                hostname: self.hostname,
                state: Provisioning {
                    cloud_provider,
                    dns_provider,
                    node_info: None,
                    entered_state_at: Instant::now(),
                    state_timeout,
                    created_dns_records: false,
                },
            }),
            Some(NodeMachineEvent::DiscoveredNode {
                discovery_data,
                cloud_provider,
            }) => NodeMachine::Exploring(Data {
                hostname: self.hostname,
                state: Exploring {
                    discovery_data,
                    cloud_provider,
                },
            }),
            _ => NodeMachine::Initializing(self),
        }
    }
}

#[derive(Debug)]
pub struct Provisioning {
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_info: Option<CloudNodeInfo>,
    entered_state_at: Instant,
    state_timeout: Duration,
    created_dns_records: bool,
}

impl MachineState for Provisioning {}

#[async_trait]
impl Handler for Data<Provisioning> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            None => {
                if self.reached_state_timeout() {
                    NodeMachine::Deprovisioning(Data {
                        hostname: self.hostname,
                        state: Deprovisioning {
                            node_info: self.state.node_info,
                        },
                    })
                } else {
                    self.provision_node().await
                }
            }
            _ => NodeMachine::Provisioning(self),
        }
    }
}

impl Data<Provisioning> {
    fn reached_state_timeout(&self) -> bool {
        Instant::now().duration_since(self.state.entered_state_at) >= self.state.state_timeout
    }

    async fn provision_node(mut self) -> NodeMachine {
        info!("Provision node {}", self.hostname);

        match self.state.node_info.as_ref() {
            None => self.create_node().await,
            Some(_) if !self.state.created_dns_records => self.create_dns_records().await,
            _ => NodeMachine::Provisioning(self),
        }
    }

    async fn create_node(self) -> NodeMachine {
        info!("Create node via CloudProvider {}", self.hostname);

        let create_node_result =
            call!(self.state.cloud_provider.create_node(self.hostname.clone())).await;

        if let Err(e) = create_node_result {
            error!("Failed to create node {} {:?}", self.hostname, e);
        }

        NodeMachine::Provisioning(Data {
            hostname: self.hostname,
            state: Provisioning {
                node_info: create_node_result.ok(),
                ..self.state
            },
        })
    }

    async fn create_dns_records(self) -> NodeMachine {
        let node_info = self.state.node_info.as_ref().unwrap();

        info!(
            "Create dns records via DnsProvider {} addresses {:?}",
            self.hostname, node_info.ip_addresses
        );

        let create_records_result = call!(self
            .state
            .dns_provider
            .create_records(self.hostname.clone(), node_info.ip_addresses.clone()))
        .await;

        if let Err(e) = create_records_result {
            error!(
                "Failed to create dns records {} addresses {:?}",
                self.hostname, node_info.ip_addresses
            );
        }

        NodeMachine::Provisioning(Data {
            state: Provisioning {
                created_dns_records: create_records_result.is_ok(),
                ..self.state
            },
            ..self
        })
    }
}

#[derive(Debug)]
pub struct Exploring {
    pub discovery_data: NodeDiscoveryData,
    cloud_provider: Addr<dyn CloudProvider>,
}

impl MachineState for Exploring {}

#[async_trait]
impl Handler for Data<Exploring> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            None => self.explore_node_info().await,
            _ => NodeMachine::Exploring(self),
        }
    }
}

impl Data<Exploring> {
    async fn explore_node_info(self) -> NodeMachine {
        let node_info = call!(self
            .state
            .cloud_provider
            .get_node_info(self.hostname.clone()))
        .await;

        if let Ok(Some(node_info)) = node_info {
            match self.state.discovery_data.state {
                NodeDiscoveryState::Ready => NodeMachine::Ready(Data {
                    hostname: self.hostname,
                    state: Ready { node_info },
                }),
                NodeDiscoveryState::Active => NodeMachine::Active(Data {
                    hostname: self.hostname,
                    state: Active { node_info },
                }),
                NodeDiscoveryState::Draining => NodeMachine::Draining(Data {
                    hostname: self.hostname,
                    state: Draining { node_info },
                }),
            }
        } else {
            error!(
                "Failed to fetch CloudNodeInfo for {}: {:?}",
                self.hostname, node_info
            );
            NodeMachine::Exploring(self)
        }
    }
}

#[derive(Debug)]
pub struct Ready {
    node_info: CloudNodeInfo,
}

impl MachineState for Ready {}

#[async_trait]
impl Handler for Data<Ready> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Active {
    node_info: CloudNodeInfo,
}

impl MachineState for Active {}

#[async_trait]
impl Handler for Data<Active> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Draining {
    node_info: CloudNodeInfo,
}

impl MachineState for Draining {}

#[async_trait]
impl Handler for Data<Draining> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Deprovisioning {
    node_info: Option<CloudNodeInfo>,
}

impl MachineState for Deprovisioning {}

#[async_trait]
impl Handler for Data<Deprovisioning> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Deprovisioned {}

impl MachineState for Deprovisioned {}

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

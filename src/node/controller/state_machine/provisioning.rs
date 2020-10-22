use super::*;
use crate::node_discovery::NodeDiscoveryState;
use log::error;

impl MachineState for Provisioning {}

#[async_trait]
impl Handler for Data<Provisioning> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            None => {
                if self.reached_state_timeout() {
                    NodeMachine::Deprovisioning(Data {
                        hostname: self.hostname,
                        node_discovery_provider: self.node_discovery_provider,
                        cloud_provider: self.cloud_provider,
                        dns_provider: self.dns_provider,
                        state: Deprovisioning {
                            node_info: self.state.node_info,
                        },
                    })
                } else {
                    self.provision_node().await
                }
            }
            Some(NodeMachineEvent::DiscoveredNode { discovery_data }) => {
                let node_info = self.state.node_info.unwrap();

                match discovery_data.state {
                    NodeDiscoveryState::Ready => NodeMachine::Ready(Data {
                        hostname: self.hostname,
                        node_discovery_provider: self.node_discovery_provider,
                        cloud_provider: self.cloud_provider,
                        dns_provider: self.dns_provider,
                        state: Ready { node_info },
                    }),
                    _ => NodeMachine::Deprovisioning(Data {
                        hostname: self.hostname,
                        node_discovery_provider: self.node_discovery_provider,
                        cloud_provider: self.cloud_provider,
                        dns_provider: self.dns_provider,
                        state: Deprovisioning {
                            node_info: Some(node_info),
                        },
                    }),
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
            call!(self.cloud_provider.create_node(self.hostname.clone())).await;

        if let Err(e) = create_node_result {
            error!("Failed to create node {} {:?}", self.hostname, e);
        }

        NodeMachine::Provisioning(Data {
            hostname: self.hostname,
            node_discovery_provider: self.node_discovery_provider,
            cloud_provider: self.cloud_provider,
            dns_provider: self.dns_provider,
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

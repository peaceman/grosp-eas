use super::*;
use std::time::Instant;

impl MachineState for Initializing {}

#[async_trait]
impl Handler for Data<Initializing> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::ProvisionNode { state_timeout }) => {
                NodeMachine::Provisioning(Data {
                    hostname: self.hostname,
                    node_discovery_provider: self.node_discovery_provider,
                    cloud_provider: self.cloud_provider,
                    dns_provider: self.dns_provider,
                    state: Provisioning {
                        node_info: None,
                        entered_state_at: Instant::now(),
                        state_timeout,
                        created_dns_records: false,
                    },
                })
            }
            Some(NodeMachineEvent::DiscoveredNode { discovery_data }) => {
                NodeMachine::Exploring(Data {
                    hostname: self.hostname,
                    node_discovery_provider: self.node_discovery_provider,
                    cloud_provider: self.cloud_provider,
                    dns_provider: self.dns_provider,
                    state: Exploring { discovery_data },
                })
            }
            _ => NodeMachine::Initializing(self),
        }
    }
}

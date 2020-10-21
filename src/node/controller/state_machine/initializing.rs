use super::*;
use std::time::Instant;

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

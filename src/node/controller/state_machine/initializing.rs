use super::*;
use std::time::Instant;

impl MachineState for Initializing {}

#[async_trait]
impl Handler for Data<Initializing> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::ProvisionNode) => NodeMachine::Provisioning(Data {
                shared: self.shared,
                state: Provisioning {
                    node_info: None,
                    entered_state_at: Instant::now(),
                    created_dns_records: false,
                },
            }),
            Some(NodeMachineEvent::DiscoveredNode { discovery_data }) => {
                NodeMachine::Exploring(Data {
                    shared: self.shared,
                    state: Exploring { discovery_data },
                })
            }
            _ => NodeMachine::Initializing(self),
        }
    }
}

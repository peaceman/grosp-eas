use super::*;
use std::time::Instant;

impl MachineState for Initializing {}

#[async_trait]
impl Handler for Data<Initializing> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::ProvisionNode) => NodeMachine::Provisioning(Data {
                shared: self.shared,
                state: Provisioning::new(),
            }),
            Some(NodeMachineEvent::DiscoveredNode { discovery_data }) => {
                NodeMachine::Exploring(Data {
                    shared: self.shared,
                    state: Exploring { discovery_data },
                })
            }
            Some(NodeMachineEvent::ExploredNode { node_info }) => NodeMachine::Discovering(Data {
                shared: self.shared,
                state: Discovering::new(node_info),
            }),
            _ => NodeMachine::Initializing(self),
        }
    }
}

use super::*;
use crate::node::discovery::NodeDiscoveryState;
use async_trait::async_trait;

impl MachineState for Discovering {}

#[async_trait]
impl Handler for Data<Discovering> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::DiscoveredNode { discovery_data }) => {
                info!(
                    "Discovered explored node {} {:?}",
                    self.shared.hostname, discovery_data
                );

                match discovery_data.state {
                    NodeDiscoveryState::Ready => NodeMachine::Ready(Data {
                        shared: self.shared,
                        state: Ready::new(self.state.node_info),
                    }),
                    NodeDiscoveryState::Active => NodeMachine::Active(Data {
                        shared: self.shared,
                        state: Active::new_marked(self.state.node_info),
                    }),
                    NodeDiscoveryState::Draining(cause) => NodeMachine::Draining(Data {
                        shared: self.shared,
                        state: Draining::new_marked(self.state.node_info, cause),
                    }),
                }
            }
            _ if self.reached_discovery_timeout() => {
                info!(
                    "Node reached discovery timeout {} {:?}",
                    self.shared.hostname, self.state.node_info
                );

                NodeMachine::Deprovisioning(Data {
                    shared: self.shared,
                    state: Deprovisioning::new(Some(self.state.node_info)),
                })
            }
            _ => NodeMachine::Discovering(self),
        }
    }
}

impl Data<Discovering> {
    fn reached_discovery_timeout(&self) -> bool {
        Instant::now().duration_since(self.state.entered_state_at)
            >= self.shared.config.discovery_timeout
    }
}

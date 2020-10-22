use super::*;

use crate::node_discovery::NodeDiscoveryState;
use act_zero::call;
use log::error;

impl MachineState for Active {}

#[async_trait]
impl Handler for Data<Active> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::DeprovisionNode { cause }) => {
                info!(
                    "Deprovision node {} cause {:?}",
                    self.shared.hostname, cause
                );

                NodeMachine::Draining(Data {
                    shared: self.shared,
                    state: Draining {
                        node_info: self.state.node_info,
                        cause,
                    },
                })
            }
            _ if !self.state.marked_as_active => self.mark_as_active().await,
            _ => NodeMachine::Active(self),
        }
    }
}

impl Data<Active> {
    async fn mark_as_active(mut self) -> NodeMachine {
        info!("Mark node as active {}", self.shared.hostname);

        let update_state_result = call!(self
            .shared
            .node_discovery_provider
            .update_state(self.shared.hostname.clone(), NodeDiscoveryState::Active))
        .await;

        if let Err(e) = update_state_result {
            error!(
                "Failed to mark node as active {} {:?}",
                self.shared.hostname, e
            );
        } else {
            self.state.marked_as_active = true;
        }

        NodeMachine::Active(self)
    }
}

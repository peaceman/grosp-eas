use super::*;
use crate::node::discovery::NodeDiscoveryState;
use act_zero::call;
use tracing::error;

impl MachineState for Exploring {}

#[async_trait]
impl Handler for Data<Exploring> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::DeprovisionNode { .. }) => {
                info!("De-provision node");

                NodeMachine::Deprovisioning(Data {
                    shared: self.shared,
                    state: Deprovisioning::new(None),
                })
            }
            Some(NodeMachineEvent::ExploredNode { node_info }) => {
                info!("Explored node");

                self.explored_node(node_info)
            }
            _ if self.reached_exploration_timeout() => {
                info!("Node reached exploration timeout");

                NodeMachine::Deprovisioning(Data {
                    shared: self.shared,
                    state: Deprovisioning::new(None),
                })
            }
            None => self.explore_node_info().await,
            _ => NodeMachine::Exploring(self),
        }
    }
}

impl Data<Exploring> {
    async fn explore_node_info(self) -> NodeMachine {
        let node_info = call!(self
            .shared
            .cloud_provider
            .get_node_info(self.shared.node.hostname.clone()))
        .await;

        if let Ok(Some(node_info)) = node_info {
            self.explored_node(node_info)
        } else {
            error!("Failed to fetch CloudNodeInfo {:?}", node_info);
            NodeMachine::Exploring(self)
        }
    }

    fn reached_exploration_timeout(&self) -> bool {
        Instant::now().duration_since(self.state.entered_state_at)
            >= self.shared.config.exploration_timeout
    }

    fn explored_node(self, node_info: CloudNodeInfo) -> NodeMachine {
        match self.state.discovery_data.state {
            NodeDiscoveryState::Ready => NodeMachine::Ready(Data {
                shared: self.shared,
                state: Ready::new(node_info, None),
            }),
            NodeDiscoveryState::Active => NodeMachine::Active(Data {
                shared: self.shared,
                state: Active::new_marked(node_info, None),
            }),
            NodeDiscoveryState::Draining(cause) => NodeMachine::Draining(Data {
                shared: self.shared,
                state: Draining::new(node_info, cause, None),
            }),
        }
    }
}

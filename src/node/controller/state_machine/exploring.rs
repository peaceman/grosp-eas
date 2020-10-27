use super::*;
use crate::node::discovery::NodeDiscoveryState;
use act_zero::call;
use log::error;

impl MachineState for Exploring {}

#[async_trait]
impl Handler for Data<Exploring> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::DeprovisionNode { .. }) => {
                info!("De-provision node {}", self.shared.hostname);

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
            .get_node_info(self.shared.hostname.clone()))
        .await;

        if let Ok(Some(node_info)) = node_info {
            match self.state.discovery_data.state {
                NodeDiscoveryState::Ready => NodeMachine::Ready(Data {
                    shared: self.shared,
                    state: Ready::new(node_info),
                }),
                NodeDiscoveryState::Active => NodeMachine::Active(Data {
                    shared: self.shared,
                    state: Active::new_marked(node_info),
                }),
                NodeDiscoveryState::Draining(cause) => NodeMachine::Draining(Data {
                    shared: self.shared,
                    state: Draining::new(node_info, cause),
                }),
            }
        } else {
            error!(
                "Failed to fetch CloudNodeInfo for {}: {:?}",
                self.shared.hostname, node_info
            );
            NodeMachine::Exploring(self)
        }
    }
}

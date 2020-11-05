use super::*;
use crate::node::discovery::NodeDiscoveryState;

impl MachineState for Ready {}

#[async_trait]
impl Handler for Data<Ready> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::ActivateNode) => {
                info!("Activate Node");

                NodeMachine::Active(Data {
                    shared: self.shared,
                    state: Active::new(self.state.node_info, self.state.stats_streamer),
                })
            }
            Some(NodeMachineEvent::DiscoveredNode {
                discovery_data:
                    NodeDiscoveryData {
                        state: NodeDiscoveryState::Ready,
                        ..
                    },
            }) => {
                info!("Discovered ready node");

                NodeMachine::Ready(Data {
                    shared: self.shared,
                    state: Ready {
                        last_discovered_at: Some(Instant::now()),
                        ..self.state
                    },
                })
            }
            _ if self.reached_discovery_timeout() => {
                info!("Reached node discovery timeout");

                NodeMachine::Deprovisioning(Data {
                    shared: self.shared,
                    state: Deprovisioning::new(Some(self.state.node_info)),
                })
            }
            _ if self.state.stats_streamer.is_none() => self.start_stats_streamer(),
            _ => NodeMachine::Ready(self),
        }
    }
}

impl Data<Ready> {
    fn reached_discovery_timeout(&self) -> bool {
        let cmp_instant = match self.state.last_discovered_at {
            Some(v) => v,
            None => self.state.entered_state_at,
        };

        Instant::now().duration_since(cmp_instant) >= self.shared.config.discovery_timeout
    }

    fn start_stats_streamer(mut self) -> NodeMachine {
        self.state.stats_streamer = Some(start_stats_streamer(&self.shared));

        NodeMachine::Ready(self)
    }
}

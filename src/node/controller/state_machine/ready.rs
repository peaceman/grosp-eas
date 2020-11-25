use super::*;
use crate::node::discovery::NodeDiscoveryState;
use act_zero::call;
use tracing::{error, info};

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
                discovery_data: NodeDiscoveryData { state, .. },
            }) => match state {
                NodeDiscoveryState::Ready => {
                    info!("Discovered ready node, updating last discovery timestamp");
                    NodeMachine::Ready(Data {
                        shared: self.shared,
                        state: Ready {
                            last_discovered_at: Some(Instant::now()),
                            ..self.state
                        },
                    })
                }
                _ => {
                    info!(
                        state = format!("{:?}", state).as_str(),
                        "Discovered ready node in unexpected state"
                    );

                    NodeMachine::Ready(self)
                }
            },
            Some(NodeMachineEvent::DeprovisionNode { .. }) => {
                info!("De-provision node");

                NodeMachine::Deprovisioning(Data {
                    shared: self.shared,
                    state: Deprovisioning::new(Some(self.state.node_info)),
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
            _ if !self.state.marked_as_ready => self.mark_as_ready().await,
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

    async fn mark_as_ready(mut self) -> NodeMachine {
        info!("Mark node as ready");

        let update_state_result = call!(self
            .shared
            .node_discovery_provider
            .update_state(self.shared.node.hostname.clone(), NodeDiscoveryState::Ready))
        .await;

        if let Err(e) = update_state_result {
            error!("Failed to mark node as ready {:?}", e);
        } else {
            self.state.marked_as_ready = true;
        }

        NodeMachine::Ready(self)
    }
}

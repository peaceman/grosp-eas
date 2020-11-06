use super::*;

use crate::node::controller::stats_streamer::StatsStreamer;
use crate::node::discovery::NodeDiscoveryState;
use act_zero::call;
use act_zero::runtimes::tokio::spawn_actor;
use tracing::error;

impl MachineState for Active {}

#[async_trait]
impl Handler for Data<Active> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::DeprovisionNode { cause }) => {
                info!("Deprovision node, cause {:?}", cause);

                NodeMachine::Draining(Data {
                    shared: self.shared,
                    state: Draining::new(self.state.node_info, cause, self.state.stats_streamer),
                })
            }
            Some(NodeMachineEvent::DiscoveredNode {
                discovery_data: NodeDiscoveryData { state, .. },
            }) => match state {
                NodeDiscoveryState::Active => {
                    info!("Discovered active node, updating last discovery timestamp");

                    NodeMachine::Active(Data {
                        shared: self.shared,
                        state: Active {
                            last_discovered_at: Some(Instant::now()),
                            ..self.state
                        },
                    })
                }
                _ => {
                    info!(
                        state = format!("{:?}", state).as_str(),
                        "Discovered active node in unexpected state"
                    );

                    NodeMachine::Active(self)
                }
            },
            _ if self.reached_discovery_timeout() => {
                info!("Reached node discovery timeout");

                NodeMachine::Deprovisioning(Data {
                    shared: self.shared,
                    state: Deprovisioning::new(Some(self.state.node_info)),
                })
            }
            _ if !self.state.marked_as_active => self.mark_as_active().await,
            _ if self.state.stats_streamer.is_none() => self.start_stats_streamer(),
            _ => NodeMachine::Active(self),
        }
    }
}

impl Data<Active> {
    async fn mark_as_active(mut self) -> NodeMachine {
        info!("Mark node as active");

        let update_state_result = call!(self.shared.node_discovery_provider.update_state(
            self.shared.node.hostname.clone(),
            NodeDiscoveryState::Active
        ))
        .await;

        if let Err(e) = update_state_result {
            error!("Failed to mark node as active {:?}", e);
        } else {
            self.state.marked_as_active = true;
        }

        NodeMachine::Active(self)
    }

    fn reached_discovery_timeout(&self) -> bool {
        let cmp_instant = match self.state.last_discovered_at {
            Some(v) => v,
            None => self.state.entered_state_at,
        };

        Instant::now().duration_since(cmp_instant) >= self.shared.config.discovery_timeout
    }

    fn start_stats_streamer(mut self) -> NodeMachine {
        self.state.stats_streamer = Some(start_stats_streamer(&self.shared));

        NodeMachine::Active(self)
    }
}

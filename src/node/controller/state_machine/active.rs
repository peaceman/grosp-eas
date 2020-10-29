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
                info!(
                    "Deprovision node {} cause {:?}",
                    self.shared.hostname, cause
                );

                NodeMachine::Draining(Data {
                    shared: self.shared,
                    state: Draining::new(self.state.node_info, cause),
                })
            }
            Some(NodeMachineEvent::DiscoveredNode {
                discovery_data:
                    NodeDiscoveryData {
                        state: NodeDiscoveryState::Active,
                        ..
                    },
            }) => {
                info!("Discovered active node {}", self.shared.hostname);

                NodeMachine::Active(Data {
                    shared: self.shared,
                    state: Active {
                        last_discovered_at: Some(Instant::now()),
                        ..self.state
                    },
                })
            }
            _ if self.reached_discovery_timeout() => {
                info!("Reached node discovery timeout {}", self.shared.hostname);

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

    fn reached_discovery_timeout(&self) -> bool {
        let cmp_instant = match self.state.last_discovered_at {
            Some(v) => v,
            None => self.state.entered_state_at,
        };

        Instant::now().duration_since(cmp_instant) >= self.shared.config.discovery_timeout
    }

    fn start_stats_streamer(mut self) -> NodeMachine {
        info!("Start stats streamer actor {}", self.shared.hostname);

        self.state.stats_streamer = Some(spawn_actor(StatsStreamer::new(
            self.shared.hostname.clone(),
            self.shared.node_stats_observer.clone(),
            self.shared.node_stats_stream_factory.clone(),
        )));

        NodeMachine::Active(self)
    }
}

use super::*;
use crate::node::discovery::NodeDiscoveryState;
use act_zero::call;
use tracing::error;

impl MachineState for Draining {}

#[async_trait]
impl Handler for Data<Draining> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match (event, &self.state.cause) {
            (None, _) if self.reached_draining_time() => {
                info!("Reached draining time of node; start de-provisioning");

                NodeMachine::Deprovisioning(Data {
                    shared: self.shared,
                    state: Deprovisioning::new(Some(self.state.node_info)),
                })
            }
            (Some(NodeMachineEvent::ActivateNode), NodeDrainingCause::Scaling) => {
                info!("Re-activate draining node");

                NodeMachine::Active(Data {
                    shared: self.shared,
                    state: Active::new(self.state.node_info, self.state.stats_streamer),
                })
            }
            _ if self.state.stats_streamer.is_none() => self.start_stats_streamer(),
            _ if !self.state.marked_as_draining => self.mark_as_draining().await,
            _ => NodeMachine::Draining(self),
        }
    }
}

impl Data<Draining> {
    fn reached_draining_time(&self) -> bool {
        Instant::now().duration_since(self.state.entered_state_at)
            >= self.shared.config.draining_time
    }

    async fn mark_as_draining(mut self) -> NodeMachine {
        info!("Mark node as draining");

        let update_state_result = call!(self.shared.node_discovery_provider.update_state(
            self.shared.node.hostname.clone(),
            NodeDiscoveryState::Draining(self.state.cause)
        ))
        .await;

        if let Err(e) = update_state_result {
            error!("Failed to mark node as draining {:?}", e);
        } else {
            self.state.marked_as_draining = true;
        }

        NodeMachine::Draining(self)
    }

    fn start_stats_streamer(mut self) -> NodeMachine {
        info!("Start stats streamer actor");

        self.state.stats_streamer = Some(start_stats_streamer(&self.shared));

        NodeMachine::Draining(self)
    }
}

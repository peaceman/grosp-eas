use super::*;

impl MachineState for Draining {}

#[async_trait]
impl Handler for Data<Draining> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match (event, &self.state.cause) {
            (None, _) if self.reached_draining_time() => {
                info!(
                    "Reached draining time of node {} start de-provisioning",
                    self.shared.hostname
                );

                NodeMachine::Deprovisioning(Data {
                    shared: self.shared,
                    state: Deprovisioning::new(Some(self.state.node_info)),
                })
            }
            (Some(NodeMachineEvent::ActivateNode), NodeDrainingCause::Scaling) => {
                info!("Re-activate draining node {}", self.shared.hostname);

                NodeMachine::Active(Data {
                    shared: self.shared,
                    state: Active::new(self.state.node_info),
                })
            }
            _ => NodeMachine::Draining(self),
        }
    }
}

impl Data<Draining> {
    fn reached_draining_time(&self) -> bool {
        Instant::now().duration_since(self.state.entered_state_at)
            >= self.shared.config.draining_time
    }
}

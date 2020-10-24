use super::*;
use async_trait::async_trait;

impl MachineState for Discovering {}

#[async_trait]
impl Handler for Data<Discovering> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
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

use super::*;

impl MachineState for Ready {}

#[async_trait]
impl Handler for Data<Ready> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::ActivateNode) => {
                info!("Activate Node {}", self.shared.hostname);

                NodeMachine::Active(Data {
                    shared: self.shared,
                    state: Active::new(self.state.node_info),
                })
            }
            _ => NodeMachine::Ready(self),
        }
    }
}

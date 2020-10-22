use super::*;

impl MachineState for Ready {}

#[async_trait]
impl Handler for Data<Ready> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            Some(NodeMachineEvent::ActivateNode) => {
                info!("Activate Node {}", self.hostname);

                NodeMachine::Active(Data {
                    node_discovery_provider: self.node_discovery_provider,
                    cloud_provider: self.cloud_provider,
                    dns_provider: self.dns_provider,
                    hostname: self.hostname,
                    state: Active {
                        node_info: self.state.node_info,
                        marked_as_active: false,
                    },
                })
            }
            _ => NodeMachine::Ready(self),
        }
    }
}

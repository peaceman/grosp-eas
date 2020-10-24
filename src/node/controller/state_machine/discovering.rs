use super::*;
use async_trait::async_trait;

impl MachineState for Discovering {}

#[async_trait]
impl Handler for Data<Discovering> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        NodeMachine::Discovering(self)
    }
}

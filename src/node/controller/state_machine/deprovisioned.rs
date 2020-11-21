use super::*;

impl MachineState for Deprovisioned {}

#[async_trait]
impl Handler for Data<Deprovisioned> {
    async fn handle(self, _event: Option<NodeMachineEvent>) -> NodeMachine {
        NodeMachine::Deprovisioned(self)
    }
}

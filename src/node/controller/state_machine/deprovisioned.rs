use super::*;

impl MachineState for Deprovisioned {}

#[async_trait]
impl Handler for Data<Deprovisioned> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        match event {
            _ => NodeMachine::Deprovisioned(self),
        }
    }
}

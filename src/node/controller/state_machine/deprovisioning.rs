use super::*;

impl MachineState for Deprovisioning {}

#[async_trait]
impl Handler for Data<Deprovisioning> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}
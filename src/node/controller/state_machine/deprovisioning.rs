use super::*;

use act_zero::call;
use async_trait::async_trait;
use tracing::{error, info};

impl MachineState for Deprovisioning {}

#[async_trait]
impl Handler for Data<Deprovisioning> {
    async fn handle(self, _event: Option<NodeMachineEvent>) -> NodeMachine {
        if let (Some(_), false) = (self.state.node_info.as_ref(), self.state.deleted_node) {
            return self.delete_node().await;
        }

        if !self.state.deleted_dns_records {
            return self.delete_dns_records().await;
        }

        NodeMachine::Deprovisioned(Data {
            shared: self.shared,
            state: Deprovisioned,
        })
    }
}

impl Data<Deprovisioning> {
    async fn delete_node(self) -> NodeMachine {
        info!("Delete node {:?}", self.state.node_info);

        let node_info = self.state.node_info.clone().unwrap();

        let result_fut = call!(self.shared.cloud_provider.delete_node(node_info));
        let result = result_fut.await;

        let deleted_node = match result {
            Ok(_) => true,
            Err(e) => {
                error!("Failed deleting node {:?} {:?}", self.state.node_info, e);

                false
            }
        };

        NodeMachine::Deprovisioning(Data {
            state: Deprovisioning {
                deleted_node,
                ..self.state
            },
            ..self
        })
    }

    async fn delete_dns_records(self) -> NodeMachine {
        info!("Delete dns records");

        let result_fut = call!(self
            .shared
            .dns_provider
            .delete_records(self.shared.node.hostname.clone()));
        let result = result_fut.await;

        let deleted_dns_records = match result {
            Ok(_) => true,
            Err(e) => {
                error!("Failed deleting dns records {:?}", e);

                false
            }
        };

        NodeMachine::Deprovisioning(Data {
            state: Deprovisioning {
                deleted_dns_records,
                ..self.state
            },
            ..self
        })
    }
}

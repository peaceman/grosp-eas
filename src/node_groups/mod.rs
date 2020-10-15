mod controller;
pub mod discovery;
mod scaler;

use serde::Deserialize;

pub use controller::NodeGroupsController;
pub use controller::NodeGroupsControllerTrait;

#[derive(Deserialize, Debug, Clone)]
pub struct NodeGroup {
    name: String,
}

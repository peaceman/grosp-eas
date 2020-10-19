mod controller;
pub mod discovery;
mod scaler;

use serde::Deserialize;

pub use controller::NodeGroupsController;

#[derive(Debug, Clone, Deserialize)]
pub struct NodeGroup {
    name: String,
}

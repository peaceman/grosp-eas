use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[serde(default)]
#[derive(Eq, Default, PartialEq, Serialize, Deserialize, Debug)]
pub struct AgentService {
    pub ID: String,
    pub Service: String,
    pub Tags: Option<Vec<String>>,
    pub Port: u16,
    pub Address: String,
    pub EnableTagOverride: bool,
    pub CreateIndex: u64,
    pub ModifyIndex: u64,
}

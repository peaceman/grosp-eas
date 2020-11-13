use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[allow(non_snake_case)]
#[serde(default)]
#[derive(Eq, Default, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct AgentService {
    pub ID: String,
    pub Service: String,
    pub Tags: Option<Vec<String>>,
    pub Port: u16,
    pub Address: String,
    pub Meta: HashMap<String, String>,
    pub EnableTagOverride: bool,
    pub CreateIndex: u64,
    pub ModifyIndex: u64,
}

#[allow(non_snake_case)]
#[serde(default)]
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct AgentCheck {
    pub Node: String,
    pub CheckID: String,
    pub Name: String,
    pub Status: String,
    pub Notes: String,
    pub Output: String,
    pub ServiceID: String,
    pub ServiceName: String,
}

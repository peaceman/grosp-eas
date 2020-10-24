use act_zero::{Actor, ActorResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct CloudNodeInfo {
    pub identifier: String,
    pub hostname: String,
    pub created_at: DateTime<Utc>,
    pub ip_addresses: Vec<IpAddr>,
}

#[async_trait]
pub trait CloudProvider: Actor {
    async fn get_node_info(&mut self, hostname: String) -> ActorResult<Option<CloudNodeInfo>>;
    async fn create_node(&mut self, hostname: String) -> ActorResult<CloudNodeInfo>;
    async fn delete_node(&mut self, node_info: CloudNodeInfo) -> ActorResult<()>;
}

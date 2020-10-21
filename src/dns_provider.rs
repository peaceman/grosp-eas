use act_zero::{Actor, ActorResult};
use async_trait::async_trait;
use std::net::IpAddr;

#[async_trait]
pub trait DnsProvider: Actor {
    async fn create_records(
        &mut self,
        hostname: String,
        ip_addresses: Vec<IpAddr>,
    ) -> ActorResult<()>;

    async fn delete_records(&mut self, hostname: String) -> ActorResult<()>;
}

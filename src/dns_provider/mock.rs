use super::DnsProvider;
use act_zero::{Actor, ActorResult, Addr, Produces};
use async_trait::async_trait;
use std::net::IpAddr;
use tracing::info;

pub struct MockDnsProvider;

#[async_trait]
impl Actor for MockDnsProvider {
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started MockDnsProvider");

        Produces::ok(())
    }
}

#[async_trait]
impl DnsProvider for MockDnsProvider {
    async fn create_records(
        &mut self,
        hostname: String,
        ip_addresses: Vec<IpAddr>,
    ) -> ActorResult<()> {
        Produces::ok(())
    }

    async fn delete_records(&mut self, hostname: String) -> ActorResult<()> {
        Produces::ok(())
    }
}

use super::DnsProvider;
use crate::actor;
use act_zero::{Actor, ActorError, ActorResult, Addr, Produces};
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

    async fn error(&mut self, error: ActorError) -> bool {
        actor::handle_error(error)
    }
}

#[async_trait]
impl DnsProvider for MockDnsProvider {
    async fn create_records(
        &mut self,
        _hostname: String,
        _ip_addresses: Vec<IpAddr>,
    ) -> ActorResult<()> {
        Produces::ok(())
    }

    async fn delete_records(&mut self, _hostname: String) -> ActorResult<()> {
        Produces::ok(())
    }
}

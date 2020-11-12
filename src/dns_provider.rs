mod mock;

use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{upcast, Actor, ActorResult, Addr};
use async_trait::async_trait;
use std::net::IpAddr;

use crate::config;
use crate::AppConfig;

#[async_trait]
pub trait DnsProvider: Actor {
    async fn create_records(
        &mut self,
        hostname: String,
        ip_addresses: Vec<IpAddr>,
    ) -> ActorResult<()>;

    async fn delete_records(&mut self, hostname: String) -> ActorResult<()>;
}

pub fn build_from_config(config: AppConfig) -> anyhow::Result<Addr<dyn DnsProvider>> {
    Ok(match &config.dns_provider {
        config::DnsProvider::Mock => upcast!(spawn_actor(mock::MockDnsProvider)),
    })
}

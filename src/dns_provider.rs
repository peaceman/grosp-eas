mod cloudflare;
mod hetzner;
mod mock;
mod record_store;

use ::cloudflare as cf;
use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{upcast, Actor, ActorResult, Addr};
use async_trait::async_trait;
use std::net::IpAddr;

use crate::config;
use crate::AppConfig;
use cf::framework::auth::Credentials;
use cf::framework::Environment;
use cf::framework::HttpApiClientConfig;

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
        config::DnsProvider::Hetzner {
            address,
            api_token,
            zone_apex,
            record_ttl,
        } => {
            let hetzner_dns_client = crate::hetzner_dns::Client::builder()
                .address(address.clone())
                .api_token(api_token.clone())
                .build()?;

            upcast!(spawn_actor(hetzner::HetznerDnsProvider::new(
                hetzner_dns_client,
                hetzner::Config {
                    record_ttl: *record_ttl,
                    zone_apex: zone_apex.clone(),
                }
            )))
        }
        config::DnsProvider::Cloudflare {
            zone_id,
            api_token,
            record_ttl,
        } => {
            let api_client = cf::framework::async_api::Client::new(
                Credentials::UserAuthToken {
                    token: api_token.clone(),
                },
                HttpApiClientConfig::default(),
                Environment::Production,
            )
            .map_err(|e| e.compat())?;

            let provider = cloudflare::CloudflareDnsProvider::new(
                api_client,
                cloudflare::Config {
                    zone_id: zone_id.clone(),
                    record_ttl: *record_ttl,
                },
            );

            upcast!(spawn_actor(provider))
        }
    })
}

pub fn record_type(ip: &IpAddr) -> &'static str {
    match ip {
        IpAddr::V4(_) => "A",
        IpAddr::V6(_) => "AAAA",
    }
}

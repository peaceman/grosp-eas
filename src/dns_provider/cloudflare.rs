use act_zero::{Actor, ActorError, ActorResult, Addr, Produces};
use anyhow::Result;
use async_trait::async_trait;
use cloudflare::framework::async_api::{ApiClient, Client};
use tracing::info;
use tracing::warn;

use crate::actor;
use crate::dns_provider::record_store::RecordStore;
use crate::dns_provider::{record_store, record_type, DnsProvider};
use cloudflare::endpoints::dns::{
    CreateDnsRecord, CreateDnsRecordParams, DeleteDnsRecord, DnsContent, DnsRecord, ListDnsRecords,
    ListDnsRecordsParams,
};
use std::net::IpAddr;

pub struct CloudflareDnsProvider {
    client: Client,
    config: Config,
    records: RecordStore<DnsRecord>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub zone_id: String,
    pub record_ttl: u32,
}

impl CloudflareDnsProvider {
    pub fn new(client: Client, config: Config) -> Self {
        Self {
            client,
            config,
            records: RecordStore::new(),
        }
    }

    async fn ensure_records_are_loaded(&mut self) -> Result<()> {
        if self.records.is_empty() {
            load_records(&self.client, &self.config, &mut self.records).await?;
        }

        Ok(())
    }
}

#[async_trait]
impl Actor for CloudflareDnsProvider {
    #[tracing::instrument(name = "CloudflareDnsProvider::started", skip(self, _addr))]
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");
        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        actor::handle_error(error)
    }
}

#[async_trait]
impl DnsProvider for CloudflareDnsProvider {
    #[tracing::instrument(name = "CloudflareDnsProvider::create_records", skip(self))]
    async fn create_records(
        &mut self,
        hostname: String,
        ip_addresses: Vec<IpAddr>,
    ) -> ActorResult<()> {
        self.ensure_records_are_loaded().await?;

        for ip in ip_addresses {
            delete_records(
                &self.client,
                &self.config,
                &mut self.records,
                &hostname,
                record_type(&ip),
            )
            .await?;

            info!("Create record {} {}", hostname, ip);

            let response = self
                .client
                .request(&CreateDnsRecord {
                    zone_identifier: &self.config.zone_id,
                    params: CreateDnsRecordParams {
                        ttl: Some(self.config.record_ttl),
                        priority: None,
                        proxied: None,
                        name: &hostname,
                        content: match ip {
                            IpAddr::V4(v) => DnsContent::A { content: v },
                            IpAddr::V6(v) => DnsContent::AAAA { content: v },
                        },
                    },
                })
                .await?;

            self.records.add(response.result);
        }

        Produces::ok(())
    }

    #[tracing::instrument(name = "CloudflareDnsProvider::delete_records", skip(self))]
    async fn delete_records(&mut self, hostname: String) -> ActorResult<()> {
        self.ensure_records_are_loaded().await?;

        for record_type in ["A", "AAAA"].iter() {
            delete_records(
                &self.client,
                &self.config,
                &mut self.records,
                &hostname,
                record_type,
            )
            .await?;
        }

        Produces::ok(())
    }
}

async fn delete_records(
    client: &Client,
    config: &Config,
    records: &mut RecordStore<DnsRecord>,
    hostname: &str,
    record_type: &str,
) -> Result<()> {
    let mut record_count = 0;

    for record in records.get(hostname, record_type) {
        info!(record = format!("{:?}", *record).as_str(), "Delete record");

        client
            .request(&DeleteDnsRecord {
                zone_identifier: &config.zone_id,
                identifier: &record.id,
            })
            .await?;

        records.remove(record.as_ref());
        record_count += 1;
    }

    if record_count == 0 {
        warn!(
            "Failed to find matching records in the store; hostname: {} record type: {}",
            hostname, record_type
        );
    }

    Ok(())
}

async fn load_records(
    client: &Client,
    config: &Config,
    store: &mut RecordStore<DnsRecord>,
) -> Result<()> {
    let mut page: u32 = 1;
    loop {
        info!("load_records: page {}", page);
        let response = client
            .request(&ListDnsRecords {
                zone_identifier: &config.zone_id,
                params: ListDnsRecordsParams {
                    page: Some(page),
                    ..Default::default()
                },
            })
            .await?;

        info!("load_records response: {:?}", response);

        if response.result.is_empty() {
            break;
        }

        response.result.into_iter().for_each(|r| store.add(r));

        page += 1;
    }

    Ok(())
}

impl record_store::Entry for DnsRecord {
    fn get_id(&self) -> &str {
        &self.id
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_type(&self) -> &str {
        match self.content {
            DnsContent::A { .. } => "A",
            DnsContent::AAAA { .. } => "AAAA",
            DnsContent::CNAME { .. } => "CNAME",
            DnsContent::NS { .. } => "NS",
            DnsContent::MX { .. } => "MX",
            DnsContent::TXT { .. } => "TXT",
        }
    }
}

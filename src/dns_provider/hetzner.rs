use crate::actor;
use crate::dns_provider::record_store::RecordStore;
use crate::dns_provider::{record_store, record_type, DnsProvider};
use crate::hetzner_dns::records::{NewRecord, Record, Records};
use crate::hetzner_dns::zones::{Zone, Zones};
use crate::hetzner_dns::Client;
use act_zero::{Actor, ActorError, ActorResult, Addr, Produces};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::net::IpAddr;
use tracing::info;

pub struct HetznerDnsProvider {
    client: Client,
    config: Config,
    zone: Option<Zone>,
    records: RecordStore<Record>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub zone_apex: String,
    pub record_ttl: u64,
}

#[async_trait]
impl Actor for HetznerDnsProvider {
    #[tracing::instrument(name = "HetznerDnsProvider::started", skip(self, _addr))]
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
impl DnsProvider for HetznerDnsProvider {
    #[tracing::instrument(name = "HetznerDnsProvider::create_records", skip(self))]
    async fn create_records(
        &mut self,
        hostname: String,
        ip_addresses: Vec<IpAddr>,
    ) -> ActorResult<()> {
        let record_name = self.gen_record_name(&hostname)?;
        self.fetch_zone_if_missing().await?;
        let zone = self.zone.as_ref().unwrap();
        if self.records.is_empty() {
            load_records(&self.client, zone, &mut self.records).await?;
        }

        for ip in ip_addresses {
            let record_type = record_type(&ip);

            for record in self.records.get(record_name, record_type) {
                info!(record = format!("{:?}", *record).as_str(), "Delete record");
                self.client.delete_record(&record.id).await?;
                self.records.remove(record.as_ref());
            }

            let ip_string = ip.to_string();
            let record = NewRecord {
                record_type,
                zone_id: &zone.id,
                name: record_name,
                value: &ip_string,
                ttl: Some(self.config.record_ttl),
            };

            info!(record = format!("{:?}", record).as_str(), "Create record");
            let record = self.client.create_record(&record).await?;
            self.records.add(record);
        }

        Produces::ok(())
    }

    #[tracing::instrument(name = "HetznerDnsProvider::delete_records", skip(self))]
    async fn delete_records(&mut self, hostname: String) -> ActorResult<()> {
        {
            self.fetch_zone_if_missing().await?;
            let zone = self.zone.as_ref().unwrap();
            if self.records.is_empty() {
                load_records(&self.client, zone, &mut self.records).await?;
            }
        }

        let record_name = self.gen_record_name(&hostname)?;
        for record_type in ["A", "AAAA"].iter() {
            for record in self.records.get(record_name, record_type) {
                info!(record = format!("{:?}", *record).as_str(), "Delete record");
                self.client.delete_record(&record.id).await?;
                self.records.remove(record.as_ref());
            }
        }

        Produces::ok(())
    }
}

impl HetznerDnsProvider {
    pub fn new(client: Client, config: Config) -> Self {
        Self {
            client,
            config,
            zone: None,
            records: RecordStore::new(),
        }
    }

    async fn fetch_zone_if_missing(&mut self) -> Result<()> {
        match self.zone.as_ref() {
            Some(_) => Ok(()),
            None => match self.client.search_zone(&self.config.zone_apex).await? {
                Some(zone) => {
                    self.zone = Some(zone);

                    Ok(())
                }
                None => Err(anyhow!("Failed to find zone {}", self.config.zone_apex)),
            },
        }
    }

    fn gen_record_name<'a>(&self, full: &'a str) -> Result<&'a str> {
        match full.strip_suffix(format!(".{}", &self.config.zone_apex).as_str()) {
            Some(name) => Ok(name),
            None => Err(anyhow!(
                "Failed to remove zone apex {}",
                &self.config.zone_apex
            )),
        }
    }
}

async fn load_records(client: &Client, zone: &Zone, store: &mut RecordStore<Record>) -> Result<()> {
    client
        .get_all_records(&zone.id)
        .await?
        .into_iter()
        .for_each(|r| store.add(r));

    Ok(())
}

impl record_store::Entry for Record {
    fn get_id(&self) -> &str {
        &self.id
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_type(&self) -> &str {
        &self.record_type
    }
}

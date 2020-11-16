use crate::dns_provider::DnsProvider;
use crate::hetzner_dns::records::{NewRecord, Record, Records};
use crate::hetzner_dns::zones::{Zone, Zones};
use crate::hetzner_dns::Client;
use act_zero::{Actor, ActorResult, Addr, Produces};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use record_store::RecordStore;
use std::collections::HashMap;
use std::net::IpAddr;
use std::rc::{Rc, Weak};
use tracing::info;

pub struct HetznerDnsProvider {
    client: Client,
    config: Config,
    zone: Option<Zone>,
    records: RecordStore,
}

#[derive(Debug, Clone)]
pub struct Config {
    zone_apex: String,
    record_ttl: u64,
}

#[async_trait]
impl Actor for HetznerDnsProvider {
    #[tracing::instrument(name = "HetznerDnsProvider::started", skip(self))]
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");
        Produces::ok(())
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
                self.client.delete_record(&record.id).await?;
                self.records.remove(record.as_ref());
            }

            let record = NewRecord {
                record_type: record_type.to_owned(),
                zone_id: zone.id.clone(),
                name: record_name.to_owned(),
                value: ip.to_string(),
                ttl: Some(self.config.record_ttl),
            };

            let record = self.client.create_record(&record).await?;
            self.records.add(record);
        }

        Produces::ok(())
    }

    #[tracing::instrument(name = "HetznerDnsProvider::delete_records", skip(self))]
    async fn delete_records(&mut self, hostname: String) -> ActorResult<()> {
        unimplemented!()
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

    async fn get_zone(&mut self) -> Result<&Zone> {
        if self.zone.is_some() {
            Ok(self.zone.as_ref().unwrap())
        } else {
            match self.client.search_zone(&self.config.zone_apex).await? {
                Some(zone) => {
                    self.zone = Some(zone);

                    Ok(self.zone.as_ref().unwrap())
                }
                None => Err(anyhow!("Failed to find zone {}", self.config.zone_apex)),
            }
        }
    }

    fn gen_record_name<'a>(&self, full: &'a str) -> Result<&'a str> {
        match full.strip_suffix(&self.config.zone_apex) {
            Some(name) => Ok(name),
            None => Err(anyhow!(
                "Failed to remove zone apex {}",
                &self.config.zone_apex
            )),
        }
    }
}

async fn load_records(client: &Client, zone: &Zone, store: &mut RecordStore) -> Result<()> {
    client
        .get_all_records(&zone.id)
        .await?
        .into_iter()
        .for_each(|r| store.add(r));

    Ok(())
}

mod record_store {
    use crate::hetzner_dns::records::Record;
    use std::collections::HashMap;
    use std::sync::{Arc, Weak};

    pub struct RecordStore {
        records: HashMap<String, Arc<Record>>,
        lookup: HashMap<String, HashMap<String, Vec<Weak<Record>>>>,
    }

    impl RecordStore {
        pub fn new() -> Self {
            Self {
                records: HashMap::new(),
                lookup: HashMap::new(),
            }
        }

        pub fn add(&mut self, record: Record) {
            let record = Arc::new(record);

            if let Some(_) = self.records.insert(record.id.clone(), Arc::clone(&record)) {
                self.remove_from_lookup(record.as_ref());
            }

            let by_type = self
                .lookup
                .entry(record.name.clone())
                .or_insert_with(|| HashMap::new());

            let record_list = by_type
                .entry(record.record_type.clone())
                .or_insert_with(|| Vec::new());

            record_list.push(Arc::downgrade(&record));
        }

        pub fn remove(&mut self, record: &Record) {
            if let Some(_) = self.records.remove(&record.id) {
                self.remove_from_lookup(record);
            }
        }

        pub fn get(&self, name: &str, record_type: &str) -> Vec<Arc<Record>> {
            self.lookup
                .get(name)
                .and_then(|by_type| by_type.get(record_type))
                .map(|list| list.iter().filter_map(|r| r.upgrade()).collect())
                .unwrap_or_else(|| vec![])
        }

        pub fn is_empty(&self) -> bool {
            self.records.is_empty()
        }

        fn remove_from_lookup(&mut self, record: &Record) {
            if let Some(by_type) = self.lookup.get_mut(&record.name) {
                if let Some(list) = by_type.get_mut(&record.record_type) {
                    list.retain(|v| match v.upgrade() {
                        Some(v) => v.id == record.id,
                        None => true,
                    });

                    if list.is_empty() {
                        by_type.remove(&record.name);
                    }

                    if by_type.is_empty() {
                        self.lookup.remove(&record.name);
                    }
                }
            }
        }
    }
}

fn record_type(ip: &IpAddr) -> &'static str {
    match ip {
        IpAddr::V4(_) => "A",
        IpAddr::V6(_) => "AAAA",
    }
}

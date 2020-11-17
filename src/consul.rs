pub mod agent;
pub mod catalog;
pub mod health;
pub mod kv;
mod request;

use anyhow::{Context, Result};
use reqwest::ClientBuilder;
use std::time::Duration;

#[derive(Debug)]
pub struct Client {
    config: Config,
    http_client: reqwest::Client,
}

impl Client {
    pub fn new(config: Config) -> Result<Self> {
        Ok(Self {
            config,
            http_client: ClientBuilder::new().build()?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub address: String,
    pub datacenter: Option<String>,
    pub wait_time: Option<Duration>,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigBuilder {
    address: Option<String>,
    datacenter: Option<String>,
    wait_time: Option<Duration>,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

impl ConfigBuilder {
    pub fn address(mut self, address: String) -> Self {
        self.address = Some(address);
        self
    }

    pub fn datacenter(mut self, datacenter: String) -> Self {
        self.datacenter = Some(datacenter);
        self
    }

    pub fn build(self) -> Result<Config> {
        Ok(Config {
            address: self.address.with_context(|| "Missing address")?,
            datacenter: self.datacenter,
            wait_time: self.wait_time,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct QueryOptions {
    pub datacenter: Option<String>,
    pub wait_index: Option<u64>,
    pub wait_time: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct QueryMeta {
    pub last_index: Option<u64>,
    pub request_time: Duration,
}

#[derive(Clone, Debug, Default)]
pub struct WriteOptions {
    pub datacenter: Option<String>,
}

#[derive(Clone, Debug)]
pub struct WriteMeta {
    pub request_time: Duration,
}

mod request;
pub mod zones;

use anyhow::{Context, Result};
use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Client {
    config: Config,
    http_client: reqwest::Client,
}

#[derive(Clone, Debug)]
struct Config {
    address: String,
    api_token: String,
}

impl Client {
    pub fn builder() -> Builder {
        Builder::default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Builder {
    address: Option<String>,
    api_token: Option<String>,
}

impl Builder {
    pub fn address(mut self, address: String) -> Self {
        self.address = Some(address);
        self
    }

    pub fn api_token(mut self, api_token: String) -> Self {
        self.api_token = Some(api_token);
        self
    }

    pub fn build(self) -> Result<Client> {
        Ok(Client {
            config: Config {
                address: self.address.with_context(|| "Missing address")?,
                api_token: self.api_token.with_context(|| "Missing api token")?,
            },
            http_client: ClientBuilder::new().build()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub last_page: u64,
    pub total_entries: u64,
}

use request::get_list;

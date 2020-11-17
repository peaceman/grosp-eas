pub mod error;
mod request;
pub mod servers;

use reqwest::ClientBuilder;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};
use std::net::Ipv6Addr;

pub type Result<T> = std::result::Result<T, error::Error>;

#[derive(Debug)]
pub struct Client {
    config: Config,
    http_client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct Config {
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
        use error::Error::*;

        Ok(Client {
            config: Config {
                address: self.address.ok_or(MissingConfig("address"))?,
                api_token: self.api_token.ok_or(MissingConfig("api_token"))?,
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
    pub last_page: Option<u64>,
    pub total_entries: Option<u64>,
}

use crate::hetzner_dns::request::get_list;
use crate::hetzner_dns::{Client, PaginationMeta};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
}

#[async_trait]
pub trait Zones {
    async fn search_zone(&self, name: &str) -> Result<Option<Zone>>;
}

#[async_trait]
impl Zones for Client {
    async fn search_zone(&self, name: &str) -> Result<Option<Zone>> {
        let mut params = HashMap::new();
        params.insert(String::from("name"), String::from(name));

        let path = format!("/api/v1/zones");
        let (mut zones, _pagination_meta): (Vec<Zone>, PaginationMeta) = get_list(
            &self.http_client,
            &self.config,
            &path,
            "/zones",
            params,
            None,
        )
        .await?;

        Ok(zones.pop())
    }
}

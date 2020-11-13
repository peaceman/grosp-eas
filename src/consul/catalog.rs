use crate::consul::agent::{AgentCheck, AgentService};
use crate::consul::request::put;
use crate::consul::{Client, WriteMeta, WriteOptions};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[allow(non_snake_case)]
#[serde(default)]
#[derive(Eq, Default, PartialEq, Serialize, Deserialize, Debug)]
pub struct CatalogRegistration {
    pub ID: String,
    pub Node: String,
    pub Address: String,
    pub TaggedAddresses: HashMap<String, String>,
    pub NodeMeta: HashMap<String, String>,
    pub Datacenter: String,
    pub Service: Option<AgentService>,
    pub Check: Option<AgentCheck>,
    pub SkipNodeUpdate: bool,
}

#[async_trait]
pub trait Catalog {
    async fn register(
        &self,
        reg: &CatalogRegistration,
        q: Option<&WriteOptions>,
    ) -> Result<((), WriteMeta)>;
}

#[async_trait]
impl Catalog for Client {
    async fn register(
        &self,
        reg: &CatalogRegistration,
        q: Option<&WriteOptions>,
    ) -> Result<((), WriteMeta)> {
        put(
            &self.http_client,
            &self.config,
            "/v1/catalog/register",
            Some(reg),
            HashMap::new(),
            q,
        )
        .await
    }
}

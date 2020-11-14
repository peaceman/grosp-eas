use crate::hetzner_dns::request::{delete, get_list, post};
use crate::hetzner_dns::{Client, PaginationMeta, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    #[serde(rename = "type")]
    pub record_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub zone_id: String,
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
}

#[async_trait]
pub trait Records {
    async fn get_all_records(&self, zone_id: &str) -> Result<Vec<Record>>;
    async fn create_record(&self, record: &Record) -> Result<()>;
    async fn delete_record(&self, record_id: &str) -> Result<()>;
}

#[async_trait]
impl Records for Client {
    async fn get_all_records(&self, zone_id: &str) -> Result<Vec<Record>> {
        let mut params = HashMap::new();
        params.insert(String::from("zone_id"), String::from(zone_id));

        let path = "/api/v1/records";
        let (records, _pagination_meta) = get_list(
            &self.http_client,
            &self.config,
            path,
            "/records",
            params,
            None,
        )
        .await?;

        Ok(records)
    }

    async fn create_record(&self, record: &Record) -> Result<()> {
        let path = "/api/v1/records";
        post(
            &self.http_client,
            &self.config,
            path,
            record,
            HashMap::new(),
        )
        .await
    }

    async fn delete_record(&self, record_id: &str) -> Result<()> {
        let path = format!("/api/v1/records/{}", record_id);
        delete(&self.http_client, &self.config, &path, HashMap::new()).await
    }
}

use crate::consul::request::get_vec;
use crate::consul::{Client, QueryMeta, QueryOptions};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[allow(non_snake_case)]
#[serde(default)]
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct KVPair {
    pub Key: String,
    pub CreateIndex: Option<u64>,
    pub ModifyIndex: Option<u64>,
    pub LockIndex: Option<u64>,
    pub Flags: Option<u64>,
    pub Value: String,
    pub Session: Option<String>,
}

#[async_trait]
pub trait KV {
    async fn list(
        &self,
        prefix: &str,
        options: Option<&QueryOptions>,
    ) -> Result<(Vec<KVPair>, QueryMeta)>;
}

#[async_trait]
impl KV for Client {
    async fn list(
        &self,
        prefix: &str,
        options: Option<&QueryOptions>,
    ) -> Result<(Vec<KVPair>, QueryMeta)> {
        let mut params = HashMap::new();
        params.insert("recurse".into(), "true".into());

        let path = format!("/v1/kv/{}", prefix);
        get_vec(&self.http_client, &self.config, &path, params, options).await
    }
}

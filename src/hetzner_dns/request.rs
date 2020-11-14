use crate::hetzner_dns::{Config, PaginationMeta, PaginationParams};
use anyhow::{anyhow, Context, Result};
use http::header::ACCEPT;
use reqwest::{RequestBuilder, Url};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

pub(in crate::hetzner_dns) async fn get_list<R: DeserializeOwned>(
    http_client: &reqwest::Client,
    config: &Config,
    path: &str,
    content_json_path: &str,
    mut params: HashMap<String, String>,
    pagination: Option<PaginationParams>,
) -> Result<(Vec<R>, Option<PaginationMeta>)> {
    if let Some(pagination) = pagination.as_ref() {
        params.paginate(pagination);
    }

    let url = format!("{}{}", config.address, path);
    let url = Url::parse_with_params(&url, params.iter())
        .with_context(|| format!("Failed to parse URL: {} Params: {:?}", url, &params))?;

    let request_builder = http_client
        .get(url)
        .with_auth(config)
        .header(ACCEPT, "application/json");
    let response = request_builder.send().await?;

    let mut json: Value = response
        .json()
        .await
        .with_context(|| "Failed to parse JSON response")?;

    let pagination: Option<PaginationMeta> = match json.pointer_mut("/meta/pagination") {
        Some(json_pagination) => serde_json::from_value(json_pagination.take())
            .with_context(|| "Failed to parse pagination json")?,
        None => None,
    };

    let data: Vec<R> = match json.pointer_mut(content_json_path) {
        Some(json_data) => serde_json::from_value(json_data.take())
            .with_context(|| "Failed to parse content json")?,
        None => return Err(anyhow!("Failed to fetch content json from response")),
    };

    Ok((data, pagination))
}

trait Paginate {
    fn paginate(&mut self, pagination: &PaginationParams);
}

impl Paginate for HashMap<String, String> {
    fn paginate(&mut self, pagination: &PaginationParams) {
        self.insert(String::from("page"), pagination.page.to_string());
        self.insert(String::from("per_page"), pagination.per_page.to_string());
    }
}

trait Authenticated {
    fn with_auth(self, config: &Config) -> Self;
}

impl Authenticated for RequestBuilder {
    fn with_auth(self, config: &Config) -> Self {
        self.header("Auth-API-Token", &config.api_token)
    }
}

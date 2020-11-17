use crate::hetzner_cloud::{error::Error, Config, PaginationMeta, PaginationParams, Result};
use http::header::ACCEPT;
use reqwest::{RequestBuilder, Url};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

pub(super) async fn get_list<R: DeserializeOwned>(
    http_client: &reqwest::Client,
    config: &Config,
    path: &str,
    result_json_path: &str,
    mut params: HashMap<String, String>,
    pagination: Option<&PaginationParams>,
) -> Result<(Vec<R>, Option<PaginationMeta>)> {
    if let Some(pagination) = pagination {
        params.paginate(pagination);
    }

    let url = gen_url(config, path, &params)?;
    let request_builder = http_client
        .get(url)
        .with_auth(config)
        .header(ACCEPT, "application/json");

    let response = request_builder.send().await?;
    let mut json: Value = response.json().await?;

    let pagination: Option<PaginationMeta> = match parse_at_pointer(&mut json, "/meta/pagination") {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => return Err(e),
        None => None,
    };

    let data: Vec<R> = match parse_at_pointer(&mut json, result_json_path) {
        Some(v) => v,
        None => return Err(Error::MissingResponseValue(result_json_path.to_owned())),
    }?;

    Ok((data, pagination))
}

pub(super) async fn post<T: Serialize, R: DeserializeOwned>(
    http_client: &reqwest::Client,
    config: &Config,
    url_path: &str,
    content: &T,
    result_json_path: Option<&str>,
    params: HashMap<String, String>,
) -> Result<R> {
    let url = gen_url(config, url_path, &params)?;
    let request_builder = http_client
        .post(url)
        .with_auth(config)
        .header(ACCEPT, "application/json")
        .json(content);

    let response = request_builder.send().await?;

    if !response.status().is_success() {
        return Err(Error::BadResponse {
            headers: response.headers().clone(),
            body: response.text().await?,
        });
    }

    let mut json: Value = response.json().await?;
    let result_json_path = result_json_path.unwrap_or("/");
    let data: R = match parse_at_pointer(&mut json, result_json_path) {
        Some(v) => v,
        None => return Err(Error::MissingResponseValue(result_json_path.to_owned())),
    }?;

    Ok(data)
}

pub(super) async fn delete(
    http_client: &reqwest::Client,
    config: &Config,
    path: &str,
    params: HashMap<String, String>,
) -> Result<()> {
    let url = gen_url(config, path, &params)?;
    let request_builder = http_client
        .delete(url)
        .with_auth(config)
        .header(ACCEPT, "application/json");

    let response = request_builder.send().await?;

    if !response.status().is_success() {
        return Err(Error::BadResponse {
            headers: response.headers().clone(),
            body: response.text().await?,
        });
    }

    Ok(())
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
        self.bearer_auth(&config.api_token)
    }
}

fn gen_url(config: &Config, path: &str, params: &HashMap<String, String>) -> Result<Url> {
    let url = format!("{}{}", config.address, path);
    Url::parse_with_params(&url, params.iter()).map_err(|e| Error::InvalidUrl {
        source: e,
        params: params.clone(),
        url: url.clone(),
    })
}

fn parse_at_pointer<T: DeserializeOwned>(json: &mut Value, pointer: &str) -> Option<Result<T>> {
    json.pointer_mut(pointer).map(|json| {
        serde_json::from_value(json.take()).map_err(|e| Error::Deserialization {
            source: e,
            content: pointer.to_owned(),
        })
    })
}

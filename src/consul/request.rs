use crate::consul::{Config, QueryMeta, QueryOptions, WriteMeta, WriteOptions};
use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client as HttpClient, RequestBuilder, Url};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

pub async fn get<R: DeserializeOwned>(
    http_client: &HttpClient,
    config: &Config,
    path: &str,
    mut params: HashMap<String, String>,
    options: Option<&QueryOptions>,
) -> Result<(R, QueryMeta)> {
    params.fill(config, options);

    let url = format!("{}{}", config.address, path);
    let url = Url::parse_with_params(&url, params.iter())
        .with_context(|| format!("Failed to parse URL: {}", url))?;

    let request_builder = http_client.get(url);
    let start = Instant::now();
    let response = request_builder.send().await?;

    let consul_index = parse_consul_index(response.headers())?;

    let json = response
        .json()
        .await
        .with_context(|| "Failed to parse JSON response")?;

    Ok((
        json,
        QueryMeta {
            last_index: consul_index,
            request_time: Instant::now() - start,
        },
    ))
}

pub async fn put<T: Serialize, R: DeserializeOwned>(
    http_client: &HttpClient,
    config: &Config,
    path: &str,
    body: Option<&T>,
    params: HashMap<String, String>,
    options: Option<&WriteOptions>,
) -> Result<(R, WriteMeta)> {
    let req = |http_client: &HttpClient, url: Url| -> RequestBuilder { http_client.put(url) };

    write_with_body(http_client, config, path, body, params, options, req).await
}

async fn write_with_body<T: Serialize, R: DeserializeOwned, F>(
    http_client: &HttpClient,
    config: &Config,
    path: &str,
    body: Option<&T>,
    mut params: HashMap<String, String>,
    options: Option<&WriteOptions>,
    req: F,
) -> Result<(R, WriteMeta)>
where
    F: Fn(&HttpClient, Url) -> RequestBuilder,
{
    params.fill(config, options);

    let url = format!("{}{}", config.address, path);
    let url = Url::parse_with_params(&url, params.iter())
        .with_context(|| format!("Failed to parse URL: {}", url))?;

    let builder = req(http_client, url);
    let builder = if let Some(b) = body {
        builder.json(b)
    } else {
        builder
    };

    let start = Instant::now();
    let response = builder.send().await?;

    let json = response
        .json()
        .await
        .with_context(|| "Failed to parse JSON Response")?;

    Ok((
        json,
        WriteMeta {
            request_time: Instant::now() - start,
        },
    ))
}

fn parse_consul_index(headers: &HeaderMap<HeaderValue>) -> Result<Option<u64>> {
    let r = headers
        .get("X-Consul-Index")
        .map(|bytes: &HeaderValue| -> Result<u64> {
            std::str::from_utf8(bytes.as_bytes())
                .with_context(|| "Failed to parse UTF-8 for last index")
                .and_then(|s: &str| -> Result<u64> {
                    s.parse()
                        .with_context(|| "Failed to parse valid number for last index")
                })
        });

    Ok(match r {
        Some(v) => Some(v?),
        None => None,
    })
}

trait Fillable<T> {
    fn fill(&mut self, config: &Config, options: Option<&T>);
}

impl Fillable<QueryOptions> for HashMap<String, String> {
    fn fill(&mut self, config: &Config, options: Option<&QueryOptions>) {
        let datacenter = options
            .and_then(|o| o.datacenter.as_ref())
            .or_else(|| config.datacenter.as_ref());

        if let Some(dc) = datacenter {
            self.insert("dc".into(), dc.into());
        }

        if let Some(options) = options {
            if let Some(index) = options.wait_index {
                self.insert("index".into(), index.to_string());
            }

            if let Some(wait_time) = options.wait_time {
                self.insert("wait".into(), format!("{}s", wait_time.as_secs()));
            }
        }
    }
}

impl Fillable<WriteOptions> for HashMap<String, String> {
    fn fill(&mut self, config: &Config, options: Option<&WriteOptions>) {
        let datacenter = options
            .and_then(|o| o.datacenter.as_ref())
            .or_else(|| config.datacenter.as_ref());

        if let Some(dc) = datacenter {
            self.insert("dc".into(), dc.into());
        }
    }
}

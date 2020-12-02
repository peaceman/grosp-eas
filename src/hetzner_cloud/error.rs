use http::{HeaderMap, StatusCode};
use std::collections::HashMap;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Missing config key {0}")]
    MissingConfig(&'static str),
    #[error("Failed to generate url {url} with params {params:?} caused by {source:?}")]
    InvalidUrl {
        url: String,
        params: HashMap<String, String>,
        source: url::ParseError,
    },
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("Received bad response with status {status:?} headers {headers:?} and body {body:?}")]
    BadResponse {
        status: StatusCode,
        headers: HeaderMap,
        body: String,
    },
    #[error("Failed to deserialize response")]
    Deserialization {
        content: String,
        source: serde_json::Error,
    },
    #[error("Missing response value {0}")]
    MissingResponseValue(String),
}

use super::Result;
use crate::hetzner_cloud::request::{delete, get_list, post};
use crate::hetzner_cloud::{Client, PaginationMeta, PaginationParams};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Server {
    pub id: u64,
    pub name: String,
    pub created: DateTime<Utc>,
    pub public_net: ServerPublicNet,
    pub labels: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerPublicNet {
    pub ipv4: Ipv4Info,
    pub ipv6: Ipv6Info,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ipv4Info {
    pub ip: Ipv4Addr,
    pub dns_ptr: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ipv6Info {
    #[serde(deserialize_with = "parse_ipv6_cidr")]
    pub ip: Ipv6Addr,
    pub dns_ptr: Vec<Ipv6Ptr>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ipv6Ptr {
    pub ip: Ipv6Addr,
    pub dns_ptr: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NewServer<'a> {
    pub name: &'a str,
    pub server_type: &'a str,
    pub image: &'a str,
    pub ssh_keys: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<&'a HashMap<String, String>>,
}

#[async_trait]
pub trait Servers {
    async fn get_all_servers(&self, label_selector: Option<&str>) -> Result<Vec<Server>>;
    async fn create_server(&self, server: &NewServer<'_>) -> Result<Server>;
    async fn delete_server(&self, server_id: u64) -> Result<()>;
}

#[async_trait]
impl Servers for Client {
    async fn get_all_servers(&self, label_selector: Option<&str>) -> Result<Vec<Server>> {
        let mut params = HashMap::new();
        if let Some(label_selector) = label_selector {
            params.insert(String::from("label_selector"), String::from(label_selector));
        }

        let mut all_servers: Option<Vec<Server>> = None;
        let mut pagination_params = PaginationParams {
            page: 1,
            per_page: 25,
        };

        loop {
            let (mut servers, pagination_meta): (Vec<Server>, Option<PaginationMeta>) = get_list(
                &self.http_client,
                &self.config,
                "/v1/servers",
                "/servers",
                params.clone(),
                Some(&pagination_params),
            )
            .await?;

            if all_servers.is_none() {
                all_servers = Some(allocate_result_vec(pagination_meta));
            }

            let result_is_empty = servers.is_empty();
            all_servers.as_mut().unwrap().append(&mut servers);

            if result_is_empty {
                break;
            } else {
                pagination_params.page += 1;
            }
        }

        Ok(all_servers.unwrap_or_default())
    }

    async fn create_server(&self, server: &NewServer<'_>) -> Result<Server> {
        post(
            &self.http_client,
            &self.config,
            "/v1/servers",
            server,
            Some("/server"),
            HashMap::new(),
        )
        .await
    }

    async fn delete_server(&self, server_id: u64) -> Result<()> {
        let path = format!("/v1/servers/{}", server_id);

        delete(&self.http_client, &self.config, &path, HashMap::new()).await
    }
}

fn allocate_result_vec<T>(pagination_meta: Option<PaginationMeta>) -> Vec<T> {
    pagination_meta
        .and_then(|m| m.total_entries)
        .map(|te| Vec::with_capacity(te as usize))
        .unwrap_or_default()
}

/// Parses an ipv6 cidr address like 2a01:4f8:c17:5038::/64 and adds an explicit ::1 if the
/// address ends in ::
fn parse_ipv6_cidr<'de, D>(deserializer: D) -> std::result::Result<Ipv6Addr, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let v = s.split('/').next().unwrap_or("");

    if !v.ends_with("::") {
        v.parse().map_err(D::Error::custom)
    } else {
        let s = format!("{}::1", v.split("::").next().unwrap_or_default());

        s.parse().map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::IntoDeserializer;
    use std::net::IpAddr;

    #[test]
    fn test_ipv6_cidr_parsing() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use serde::de::value::Error as ValueError;
        let deserializer = "2a01:4f8:c17:5038::/64".to_owned().into_deserializer();
        let ip: std::result::Result<Ipv6Addr, ValueError> = parse_ipv6_cidr(deserializer);

        let expected: Ipv6Addr = "2a01:4f8:c17:5038::1".parse()?;
        assert_eq!(expected, ip.unwrap());

        Ok(())
    }

    #[test]
    fn test_ipv6_parsing_full() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use serde::de::value::Error as ValueError;
        let deserializer = "2a01:4f8:c17:5038::23".to_owned().into_deserializer();
        let ip: std::result::Result<Ipv6Addr, ValueError> = parse_ipv6_cidr(deserializer);

        let expected: Ipv6Addr = "2a01:4f8:c17:5038::23".parse()?;
        assert_eq!(expected, ip.unwrap());

        Ok(())
    }

    #[test]
    fn test_ipv6_parsing_without_cidr() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use serde::de::value::Error as ValueError;
        let deserializer = "2a01:4f8:c17:5038::".to_owned().into_deserializer();
        let ip: std::result::Result<Ipv6Addr, ValueError> = parse_ipv6_cidr(deserializer);

        let expected: Ipv6Addr = "2a01:4f8:c17:5038::1".parse()?;
        assert_eq!(expected, ip.unwrap());

        Ok(())
    }
}

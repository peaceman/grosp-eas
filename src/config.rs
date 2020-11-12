use crate::AppConfig;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Duration;

#[derive(Deserialize)]
pub struct Config {
    pub node_stats: NodeStats,
    // pub node_group_discovery: NodeGroupDiscovery,
    pub node_discovery: NodeDiscovery,
    pub node_exploration: NodeExploration,
    pub node_discovery_provider: NodeDiscoveryProvider,
    pub cloud_provider: CloudProvider,
    pub dns_provider: DnsProvider,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeStats {
    File {
        #[serde(with = "humantime_serde")]
        interval: Duration,
        path: String,
    },
    NSS {
        tls: NodeStatsNSSTLS,
        port: u16,
    },
}

#[derive(Deserialize)]
pub struct NodeStatsNSSTLS {
    pub ca_cert_path: String,
    pub client_cert_path: String,
    pub client_key_path: String,
    pub target_sni_name: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeDiscoveryProvider {
    Mock,
    File { path: String },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum NodeGroupDiscovery {
    File { interval: Duration, path: String },
}

#[derive(Deserialize)]
pub struct NodeDiscovery {
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
}

#[derive(Deserialize)]
pub struct NodeExploration {
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CloudProvider {
    File {
        exploration_path: String,
        discovery_path: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DnsProvider {
    Mock,
}

pub fn load_config() -> anyhow::Result<AppConfig> {
    let config_path = get_config_path()?;
    let file = File::open(config_path)?;

    Ok(Arc::new(serde_yaml::from_reader(BufReader::new(file))?))
}

fn get_config_path() -> anyhow::Result<String> {
    use std::env;
    use tracing::info;

    env::var("APP_CONFIG").or_else(|e| {
        info!(
            error = format!("{:?}", e).as_str(),
            "Missing or invalid APP_CONFIG env var, fallback to config.yml"
        );
        Ok("config.yml".to_string())
    })
}

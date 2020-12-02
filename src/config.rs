use crate::AppConfig;
use anyhow::Context;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Duration;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub node_stats: NodeStats,
    pub node_group_discovery: NodeGroupDiscovery,
    pub node_discovery: NodeDiscovery,
    pub node_exploration: NodeExploration,
    pub node_discovery_provider: NodeDiscoveryProvider,
    pub node_group_discovery_providers: Vec<NodeGroupDiscoveryProvider>,
    pub cloud_provider: CloudProvider,
    pub dns_provider: DnsProvider,
    pub cloud_init: CloudInit,
    pub node_group_scaler: NodeGroupScaler,
    #[serde(with = "humantime_serde")]
    pub node_group_discovery_timeout: Duration,
    pub node_controller: NodeController,
}

#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug)]
pub struct NodeStatsNSSTLS {
    pub ca_cert_path: String,
    pub client_cert_path: String,
    pub client_key_path: String,
    pub target_sni_name: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeDiscoveryProvider {
    Mock,
    File {
        path: String,
    },
    Consul {
        service_name: String,
        address: String,
    },
}

#[derive(Deserialize, Debug)]
pub struct NodeGroupDiscovery {
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
}

#[derive(Deserialize, Debug)]
pub struct NodeDiscovery {
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
}

#[derive(Deserialize, Debug)]
pub struct NodeExploration {
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CloudProvider {
    File {
        exploration_path: String,
        discovery_path: String,
    },
    Hetzner {
        server_type: String,
        image: String,
        ssh_keys: Vec<String>,
        group_label_name: String,
        api_address: String,
        api_token: String,
        location: Option<String>,
    },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DnsProvider {
    Mock,
    Hetzner {
        zone_apex: String,
        record_ttl: u64,
        api_token: String,
        address: String,
    },
    Cloudflare {
        zone_id: String,
        api_token: String,
        record_ttl: u32,
    },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeGroupDiscoveryProvider {
    File { path: String },
    Consul { key_prefix: String, address: String },
}

#[derive(Clone, Deserialize, Debug)]
pub struct CloudInit {
    pub user_data_base_file_path: String,
    pub extra_vars_base_file_path: String,
    pub extra_vars_destination_path: String,
    pub user_data_files: Vec<CloudInitUserDataFile>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct CloudInitUserDataFile {
    pub source: String,
    pub destination: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct NodeGroupScaler {
    pub node_hostname_suffix: String,
    pub scale_lock_timeout_s: u64,
    #[serde(with = "humantime_serde")]
    pub startup_cooldown: Duration,
}

#[derive(Clone, Deserialize, Debug)]
pub struct NodeController {
    #[serde(with = "humantime_serde")]
    pub draining_time: Duration,
    #[serde(with = "humantime_serde")]
    pub provisioning_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub discovery_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub exploration_timeout: Duration,
}

pub fn load_config() -> anyhow::Result<AppConfig> {
    let config_path = get_config_path()?;
    let file = File::open(&config_path)
        .with_context(|| format!("Failed to open config file {}", &config_path))?;

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

use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
pub struct Config {
    node_stats: NodeStats,
    node_discovery: NodeDiscovery,
    node_group_discovery: NodeGroupDiscovery,
    cloud_provider: CloudProvider,
    dns_provider: DnsProvider,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum NodeStats {
    File { interval: Duration, path: String },
    NSS { tls: NodeStatsNSSTLS, port: u16 },
}

#[derive(Deserialize)]
pub struct NodeStatsNSSTLS {
    ca_cert_path: String,
    client_cert_path: String,
    client_key_path: String,
    target_sni_name: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum NodeDiscovery {
    File { interval: Duration, path: String },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum NodeGroupDiscovery {
    File { interval: Duration, path: String },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum CloudProvider {
    File {
        exploration_path: String,
        discovery_path: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum DnsProvider {
    Mock,
}

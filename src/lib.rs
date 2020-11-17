use crate::config::Config;
use crate::node::NodeStats;
use std::sync::Arc;
use tokio::stream::Stream;

pub mod cloud_provider;
pub mod config;
pub mod consul;
pub mod dns_provider;
pub mod hetzner_cloud;
pub mod hetzner_dns;
pub mod node;
pub mod node_groups;
pub mod utils;

type AppConfig = Arc<Config>;

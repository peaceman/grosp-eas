use crate::config::Config;
use std::sync::Arc;

pub mod actor;
pub mod cloud_init;
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

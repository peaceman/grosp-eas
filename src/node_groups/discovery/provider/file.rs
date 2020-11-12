use tracing::{info, Instrument};

use crate::node_groups::discovery::provider::NodeGroupDiscoveryProvider;
use crate::node_groups::discovery::NodeGroupDiscoveryObserver;
use crate::node_groups::NodeGroup;
use crate::utils;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use anyhow::Context;
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::TryFutureExt;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::DirEntry;

pub struct FileNodeGroupDiscovery {
    directory_path: PathBuf,
    addr: WeakAddr<Self>,
}

impl FileNodeGroupDiscovery {
    pub fn new(directory_path: impl AsRef<Path>) -> Self {
        FileNodeGroupDiscovery {
            directory_path: directory_path.as_ref().into(),
            addr: Default::default(),
        }
    }
}

#[async_trait]
impl Actor for FileNodeGroupDiscovery {
    #[tracing::instrument(
        name = "FileNodeGroupDiscovery::started"
        skip(self, addr),
        fields(path = %self.directory_path.display())
    )]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

        self.addr = addr.downgrade();

        Produces::ok(())
    }
}

impl Drop for FileNodeGroupDiscovery {
    fn drop(&mut self) {
        info!("Drop {}", self);
    }
}

#[async_trait]
impl NodeGroupDiscoveryProvider for FileNodeGroupDiscovery {
    #[tracing::instrument(
        name = "FileNodeGroupDiscovery::discover_node_groups"
        skip(self),
        fields(path = %self.directory_path.display())
    )]
    async fn discover_node_groups(&mut self) -> ActorResult<Vec<NodeGroup>> {
        Produces::ok(scan_for_node_groups(&self.directory_path).await)
    }
}

impl fmt::Display for FileNodeGroupDiscovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FileNodeGroupDiscovery ({})",
            self.directory_path.to_string_lossy()
        )
    }
}

impl fmt::Debug for FileNodeGroupDiscovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

fn parse_node_group_file(path: impl AsRef<Path>) -> anyhow::Result<NodeGroup> {
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let result = serde_yaml::from_reader(reader)
        .with_context(|| format!("Path {}", path.as_ref().display()))?;

    Ok(result)
}

async fn scan_for_node_groups(path: impl AsRef<Path>) -> Vec<NodeGroup> {
    utils::scan_for_files(&path)
        .and_then(|files| utils::parse_files(files, parse_node_group_file))
        .await
        .unwrap_or_else(|_| vec![])
}

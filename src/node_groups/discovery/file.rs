use log::info;

use crate::node_groups::discovery::NodeGroupDiscoveryObserver;
use crate::node_groups::NodeGroup;
use crate::utils;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::TryFutureExt;
use std::fmt::Formatter;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::DirEntry;

pub struct FileNodeGroupDiscovery {
    directory_path: PathBuf,
    discovery_observer: Addr<dyn NodeGroupDiscoveryObserver>,
    timer: Timer,
    addr: WeakAddr<Self>,
}

impl FileNodeGroupDiscovery {
    pub fn new(
        directory_path: impl AsRef<Path>,
        discovery_observer: Addr<dyn NodeGroupDiscoveryObserver>,
    ) -> Self {
        FileNodeGroupDiscovery {
            directory_path: directory_path.as_ref().into(),
            discovery_observer,
            timer: Default::default(),
            addr: Default::default(),
        }
    }
}

#[async_trait]
impl Actor for FileNodeGroupDiscovery {
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started {}", self);

        self.addr = addr.downgrade();

        self.timer
            .set_interval_weak(self.addr.clone(), Duration::from_secs(5));

        Produces::ok(())
    }
}

#[async_trait]
impl Tick for FileNodeGroupDiscovery {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.discover());
        }

        Produces::ok(())
    }
}

impl Drop for FileNodeGroupDiscovery {
    fn drop(&mut self) {
        info!("Drop {}", self);
    }
}

impl FileNodeGroupDiscovery {
    async fn discover(&self) {
        info!("Start discovery {}", self);

        let node_groups = scan_for_node_groups(&self.directory_path).await;

        for node_group in node_groups.into_iter() {
            info!("Discovered node group: {:?}", node_group);
            send!(self
                .discovery_observer
                .observe_node_group_discovery(node_group));
        }

        info!("Finished discovery {}", self);
    }
}

impl std::fmt::Display for FileNodeGroupDiscovery {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileNodeGroupDiscovery ({})",
            self.directory_path.to_string_lossy()
        )
    }
}

fn parse_node_group_file(path: impl AsRef<Path>) -> anyhow::Result<NodeGroup> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let result = serde_yaml::from_reader(reader)?;

    Ok(result)
}

async fn scan_for_node_groups(path: impl AsRef<Path>) -> Vec<NodeGroup> {
    utils::scan_for_files(&path)
        .and_then(|files| utils::parse_files(files, parse_node_group_file))
        .await
        .unwrap_or_else(|_| vec![])
}

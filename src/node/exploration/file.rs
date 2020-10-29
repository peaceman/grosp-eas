use crate::cloud_provider::CloudNodeInfo;
use crate::node::exploration::NodeExplorationObserver;
use crate::utils;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use anyhow::Context;
use async_trait::async_trait;
use futures::TryFutureExt;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::info;

pub struct FileNodeExploration {
    directory_path: PathBuf,
    exploration_observer: Addr<dyn NodeExplorationObserver>,
    timer: Timer,
    addr: WeakAddr<Self>,
}

impl FileNodeExploration {
    pub fn new(
        directory_path: impl AsRef<Path>,
        exploration_observer: Addr<dyn NodeExplorationObserver>,
    ) -> Self {
        Self {
            directory_path: directory_path.as_ref().into(),
            exploration_observer,
            timer: Default::default(),
            addr: Default::default(),
        }
    }
}

#[async_trait]
impl Actor for FileNodeExploration {
    #[tracing::instrument(
        name = "FileNodeExploration::started"
        skip(self, addr),
        fields(path = %self.directory_path.display())
    )]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

        self.addr = addr.downgrade();

        self.timer
            .set_interval_weak(self.addr.clone(), Duration::from_secs(5));

        Produces::ok(())
    }
}

impl fmt::Display for FileNodeExploration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FileNodeExploration ({})",
            self.directory_path.to_string_lossy()
        )
    }
}

impl fmt::Debug for FileNodeExploration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[async_trait]
impl Tick for FileNodeExploration {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.explore());
        }

        Produces::ok(())
    }
}

impl FileNodeExploration {
    #[tracing::instrument(
        name = "FileNodeExploration::explore"
        skip(self),
        fields(path = %self.directory_path.display())
    )]
    async fn explore(&mut self) {
        let nodes = scan_for_nodes(&self.directory_path).await;

        for node in nodes {
            info!("Explored node {:?}", node);
            send!(self.exploration_observer.observe_node_exploration(node));
        }
    }
}

async fn scan_for_nodes(path: impl AsRef<Path>) -> Vec<CloudNodeInfo> {
    utils::scan_for_files(&path)
        .and_then(|files| utils::parse_files(files, parse_node_discovery_file))
        .await
        .unwrap_or_else(|_| vec![])
}

fn parse_node_discovery_file(path: impl AsRef<Path>) -> anyhow::Result<CloudNodeInfo> {
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let result = serde_yaml::from_reader(reader)
        .with_context(|| format!("Path {}", path.as_ref().display()))?;

    Ok(result)
}

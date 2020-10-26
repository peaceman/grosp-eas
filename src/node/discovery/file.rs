use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryObserver};
use crate::utils;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use futures::TryFutureExt;
use log::{error, info};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct FileNodeDiscovery {
    directory_path: PathBuf,
    discovery_observer: Addr<dyn NodeDiscoveryObserver>,
    timer: Timer,
    addr: WeakAddr<Self>,
}

impl FileNodeDiscovery {
    pub fn new(
        directory_path: impl AsRef<Path>,
        discovery_observer: Addr<dyn NodeDiscoveryObserver>,
    ) -> Self {
        Self {
            directory_path: directory_path.as_ref().into(),
            discovery_observer,
            timer: Default::default(),
            addr: Default::default(),
        }
    }
}

#[async_trait]
impl Actor for FileNodeDiscovery {
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

impl fmt::Display for FileNodeDiscovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FileNodeDiscovery ({})",
            self.directory_path.to_string_lossy()
        )
    }
}

#[async_trait]
impl Tick for FileNodeDiscovery {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.discover());
        }

        Produces::ok(())
    }
}

impl FileNodeDiscovery {
    async fn discover(&mut self) {
        info!("Start discovery {}", self);

        let node_discoveries = scan_for_node_discoveries(&self.directory_path).await;

        for node_discovery in node_discoveries {
            info!("Discovered node {:?}", node_discovery);
            send!(self
                .discovery_observer
                .observe_node_discovery(node_discovery));
        }

        info!("Finished discovery {}", self);
    }
}

async fn scan_for_node_discoveries(path: impl AsRef<Path>) -> Vec<NodeDiscoveryData> {
    utils::scan_for_files(&path)
        .and_then(|files| utils::parse_files(files, parse_node_discovery_file))
        .await
        .unwrap_or_else(|_| vec![])
}

fn parse_node_discovery_file(path: impl AsRef<Path>) -> anyhow::Result<NodeDiscoveryData> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let result = serde_yaml::from_reader(reader)?;

    Ok(result)
}

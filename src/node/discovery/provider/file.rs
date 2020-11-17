use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryProvider, NodeDiscoveryState};
use crate::utils;
use crate::utils::path_append;
use act_zero::{Actor, ActorResult, Addr, Produces, WeakAddr};
use anyhow::Context;
use async_trait::async_trait;
use futures::TryFutureExt;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use tracing::{error, info};

pub struct FileNodeDiscovery {
    directory_path: PathBuf,
    addr: WeakAddr<Self>,
}

impl FileNodeDiscovery {
    pub fn new(directory_path: impl AsRef<Path>) -> Self {
        Self {
            directory_path: directory_path.as_ref().into(),
            addr: Default::default(),
        }
    }
}

#[async_trait]
impl Actor for FileNodeDiscovery {
    #[tracing::instrument(
        name = "FileNodeDiscovery::started"
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

impl fmt::Display for FileNodeDiscovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FileNodeDiscovery ({})",
            self.directory_path.to_string_lossy()
        )
    }
}

impl fmt::Debug for FileNodeDiscovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[async_trait]
impl NodeDiscoveryProvider for FileNodeDiscovery {
    #[tracing::instrument(
        name = "FileNodeDiscovery::discover_nodes"
        skip(self),
        fields(path = %self.directory_path.display())
    )]
    async fn discover_nodes(&mut self) -> ActorResult<Vec<NodeDiscoveryData>> {
        let node_discoveries = scan_for_node_discoveries(&self.directory_path).await;

        Produces::ok(node_discoveries)
    }

    #[tracing::instrument(
        name = "FileNodeDiscovery::update_state"
        skip(self),
        fields(path = %self.directory_path.display())
    )]
    async fn update_state(
        &mut self,
        hostname: String,
        state: NodeDiscoveryState,
    ) -> ActorResult<()> {
        info!("Updating state of node {} {:?}", hostname, state);

        let path = path_append(&self.directory_path.join(&hostname), ".yml");
        let result = File::open(&path)
            .with_context(|| format!("Failed to open {} for reading", &path.display()))
            .map(BufReader::new)
            .and_then(|reader| serde_yaml::from_reader(reader).map_err(anyhow::Error::new))
            .and_then(|discovery_data: NodeDiscoveryData| {
                File::create(&path)
                    .with_context(|| format!("Failed to open {} for writing", &path.display()))
                    .map(BufWriter::new)
                    .map(|writer| (discovery_data, writer))
            })
            .and_then(|(discovery_data, writer)| {
                serde_yaml::to_writer(writer, &discovery_data).map_err(anyhow::Error::new)
            });

        if let Err(e) = result {
            error!(error = format!("{:?}", e).as_str(), "Failed updating");
        }

        Produces::ok(())
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

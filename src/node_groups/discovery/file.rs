use log::info;

use crate::node_groups::{NodeGroup, NodeGroupsController};
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

pub struct FileBasedNodeGroupExplorer {
    directory_path: PathBuf,
    node_groups_controller: Addr<NodeGroupsController>,
    timer: Timer,
    addr: WeakAddr<Self>,
}

impl FileBasedNodeGroupExplorer {
    pub fn new(
        directory_path: impl AsRef<Path>,
        node_groups_controller: Addr<NodeGroupsController>,
    ) -> Self {
        FileBasedNodeGroupExplorer {
            directory_path: directory_path.as_ref().into(),
            node_groups_controller,
            timer: Default::default(),
            addr: Default::default(),
        }
    }
}

#[async_trait]
impl Actor for FileBasedNodeGroupExplorer {
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
impl Tick for FileBasedNodeGroupExplorer {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.discover());
        }

        Produces::ok(())
    }
}

impl Drop for FileBasedNodeGroupExplorer {
    fn drop(&mut self) {
        info!("Drop {}", self);
    }
}

impl FileBasedNodeGroupExplorer {
    async fn discover(&self) {
        info!("Start discovery");

        let node_groups = scan_for_node_groups(&self.directory_path).await;

        for node_group in node_groups.into_iter() {
            info!("Discovered node group: {:?}", node_group);
            send!(self
                .node_groups_controller
                .discovered_node_group(node_group));
        }

        info!("Finished discovery");
    }
}

impl std::fmt::Display for FileBasedNodeGroupExplorer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileBasedNodeGroupExplorer ({})",
            self.directory_path.to_string_lossy()
        )
    }
}

async fn parse_node_group_files(files: Vec<DirEntry>) -> anyhow::Result<Vec<NodeGroup>> {
    let mut handles = files
        .iter()
        .map(|file| {
            let path = file.path();
            tokio::task::spawn_blocking(move || parse_node_group_file(path))
        })
        .collect::<FuturesUnordered<_>>();

    let mut node_groups = vec![];

    while let Some(join_handle_result) = handles.next().await {
        if let Ok(Ok(node_group)) = join_handle_result {
            node_groups.push(node_group);
        }
    }

    Ok(node_groups)
}

fn parse_node_group_file(path: impl AsRef<Path>) -> anyhow::Result<NodeGroup> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let result = serde_yaml::from_reader(reader)?;

    Ok(result)
}

async fn scan_for_node_groups(path: impl AsRef<Path>) -> Vec<NodeGroup> {
    scan_for_files(&path)
        .and_then(parse_node_group_files)
        .await
        .unwrap_or_else(|_| vec![])
}

async fn scan_for_files(path: impl AsRef<Path>) -> anyhow::Result<Vec<DirEntry>> {
    let dir_entries = tokio::fs::read_dir(path).await?;

    let files_in_folder = dir_entries
        .filter_map(|e| async {
            if let Ok(dir_entry) = e {
                dir_entry
                    .file_type()
                    .await
                    .ok()
                    .filter(|file_type| file_type.is_file())
                    .map(|_| dir_entry)
            } else {
                None
            }
        })
        .collect()
        .await;

    Ok(files_in_folder)
}

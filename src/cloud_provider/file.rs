use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use act_zero::{Actor, ActorResult, Addr, Produces, WeakAddr};
use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use tracing::error;
use tracing::info;

pub struct FileCloudProvider {
    directory: PathBuf,
    addr: WeakAddr<Self>,
}

impl FileCloudProvider {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            directory: directory.as_ref().into(),
            addr: Default::default(),
        }
    }
}

#[async_trait]
impl Actor for FileCloudProvider {
    #[tracing::instrument(
        name = "FileCloudProvider::started",
        skip(self, addr),
        fields(
            directory = %self.directory.display(),
        )
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

#[async_trait]
impl CloudProvider for FileCloudProvider {
    #[tracing::instrument(name = "FileCloudProvider::get_node_info", skip(self))]
    async fn get_node_info(&mut self, hostname: String) -> ActorResult<Option<CloudNodeInfo>> {
        let path = path_append(self.directory.join(hostname), ".yml");
        let node_info: anyhow::Result<CloudNodeInfo> = File::open(&path)
            .with_context(|| format!("Failed to open {}", &path.display()))
            .map(BufReader::new)
            .and_then(|reader| serde_yaml::from_reader(reader).map_err(anyhow::Error::new));

        if let Err(e) = node_info.as_ref() {
            error!("{:?}", e);
        }

        Produces::ok(node_info.ok())
    }

    #[tracing::instrument(name = "FileCloudProvider::create_node", skip(self))]
    async fn create_node(&mut self, hostname: String) -> ActorResult<CloudNodeInfo> {
        let node_info = CloudNodeInfo {
            identifier: format!("{}-identifier", hostname),
            hostname: hostname.clone(),
            group: "topkek".into(),
            created_at: Utc::now(),
            ip_addresses: vec!["1.2.3.4".parse().unwrap()],
        };

        let path = path_append(self.directory.join(hostname), ".yml");
        let des_result = File::create(&path)
            .with_context(|| format!("Failed to create {}", &path.display()))
            .map(BufWriter::new)
            .and_then(|writer| {
                serde_yaml::to_writer(writer, &node_info).map_err(anyhow::Error::new)
            });

        match des_result {
            Ok(_) => Produces::ok(node_info),
            Err(e) => {
                error!("{:?}", e);
                Err(e.into())
            }
        }
    }

    // #[tracing::instrument(name = "FileCloudProvider::delete_node", skip(self))]
    async fn delete_node(&mut self, node_info: CloudNodeInfo) -> ActorResult<()> {
        match std::fs::remove_file(path_append(self.directory.join(node_info.hostname), ".yml")) {
            Ok(_) => Produces::ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

fn path_append(path: impl AsRef<Path>, append: &str) -> PathBuf {
    let mut os = path.as_ref().to_path_buf().into_os_string();
    os.push(append);

    PathBuf::from(os)
}

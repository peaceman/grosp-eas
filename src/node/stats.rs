mod file;
mod nss;

use crate::node::NodeStats;
use std::fmt::Debug;
use tokio::stream::Stream;

use crate::config;
use crate::AppConfig;
pub use file::FileNodeStatsStream;
pub use file::FileNodeStatsStreamFactory;
pub use nss::NSSStreamFactory;
use std::fs;
use std::pin::Pin;

pub trait NodeStatsStreamFactory: Send + Sync + CloneNodeStatsStreamFactory + Debug {
    fn create_stream(&self, hostname: String) -> Pin<Box<dyn Stream<Item = NodeStats> + Send>>;
}

pub trait CloneNodeStatsStreamFactory {
    fn clone_boxed(&self) -> Box<dyn NodeStatsStreamFactory>;
}

impl<T> CloneNodeStatsStreamFactory for T
where
    T: NodeStatsStreamFactory + Clone + 'static,
{
    fn clone_boxed(&self) -> Box<dyn NodeStatsStreamFactory> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn NodeStatsStreamFactory> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

pub fn build_stream_factory_from_config(
    config: AppConfig,
) -> anyhow::Result<Box<dyn NodeStatsStreamFactory>> {
    match &config.node_stats {
        config::NodeStats::File { interval, path } => Ok(Box::new(
            FileNodeStatsStreamFactory::new(path.clone(), interval.clone()),
        )),
        config::NodeStats::NSS { tls, port } => Ok(Box::new(NSSStreamFactory::new(
            fs::read(&tls.ca_cert_path)?,
            fs::read(&tls.client_cert_path)?,
            fs::read(&tls.client_key_path)?,
            tls.target_sni_name.clone(),
            port.clone(),
        ))),
    }
}

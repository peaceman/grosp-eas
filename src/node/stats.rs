mod file;
mod nss;

use crate::node::NodeStats;
use std::fmt::Debug;
use tokio::stream::Stream;

pub use file::FileNodeStatsStream;
pub use file::FileNodeStatsStreamFactory;
pub use nss::NSSStreamFactory;
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

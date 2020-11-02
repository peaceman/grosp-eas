mod file;

use crate::node::NodeStats;
use std::fmt::Debug;
use tokio::stream::Stream;

pub use file::FileNodeStatsStream;

pub trait NodeStatsStreamFactory: Send + Sync + CloneNodeStatsStreamFactory + Debug {
    fn create_stream(&self, hostname: String) -> Box<dyn Stream<Item = NodeStats> + Unpin + Send>;
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

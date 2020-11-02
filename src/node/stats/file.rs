use crate::node::discovery::FileNodeDiscovery;
use crate::node::stats::NodeStatsStreamFactory;
use crate::node::NodeStats;
use std::fs::File;
use std::future::Future;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::stream::Stream;
use tracing::info;

#[derive(Clone, Debug)]
pub struct FileNodeStatsStreamFactory;

impl NodeStatsStreamFactory for FileNodeStatsStreamFactory {
    fn create_stream(&self, hostname: String) -> Box<dyn Stream<Item = NodeStats> + Unpin + Send> {
        info!("Creating NodeStatsStream for {}", hostname);
        Box::new(FileNodeStatsStream::new(format!(
            "test_files/node_stats/{}.yml",
            hostname
        )))
    }
}

pub struct FileNodeStatsStream {
    path: PathBuf,
    interval: tokio::time::Interval,
    read_stats_fut: Option<Pin<Box<dyn Future<Output = anyhow::Result<NodeStats>> + Send>>>,
}

impl FileNodeStatsStream {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().into(),
            interval: tokio::time::interval(Duration::from_secs(5)),
            read_stats_fut: None,
        }
    }
}

impl Stream for FileNodeStatsStream {
    type Item = NodeStats;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.read_stats_fut.is_none() {
            match Pin::new(&mut self.interval).poll_next(cx) {
                Poll::Ready(_) => {
                    self.read_stats_fut = Some(Box::pin(parse_node_stats(self.path.clone())));
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        match self.read_stats_fut.as_mut() {
            Some(rsf) => match rsf.as_mut().poll(cx) {
                Poll::Ready(ns) => {
                    self.read_stats_fut = None;

                    match ns {
                        Ok(ns) => Poll::Ready(Some(ns)),
                        Err(_) => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                    }
                }
                Poll::Pending => Poll::Pending,
            },
            None => Poll::Pending,
        }
    }
}

async fn parse_node_stats(path: impl AsRef<Path>) -> anyhow::Result<NodeStats> {
    use anyhow::Context;

    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let result = serde_yaml::from_reader(reader)
        .with_context(|| format!("Path {}", path.as_ref().display()))?;

    Ok(result)
}

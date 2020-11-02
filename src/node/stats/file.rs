use crate::node::discovery::FileNodeDiscovery;
use crate::node::NodeStats;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::stream::Stream;

pub struct FileNodeStatsStream {
    path: PathBuf,
    interval: tokio::time::Interval,
    read_stats_fut: Option<Pin<Box<dyn Future<Output = NodeStats>>>>,
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
        println!("poll next");

        if self.read_stats_fut.is_none() {
            match Pin::new(&mut self.interval).poll_next(cx) {
                Poll::Ready(_) => {
                    println!("poll tick ready");
                    self.read_stats_fut = Some(Box::pin(read_stats()));
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        match self.read_stats_fut.as_mut() {
            Some(rsf) => match rsf.as_mut().poll(cx) {
                Poll::Ready(ns) => {
                    self.read_stats_fut = None;
                    Poll::Ready(Some(ns))
                }
                Poll::Pending => Poll::Pending,
            },
            None => Poll::Pending,
        }
    }
}

async fn read_stats() -> NodeStats {
    println!("start read stats");
    tokio::time::delay_for(Duration::from_secs(3)).await;
    println!("finished read stats");
    NodeStats {
        rx_bps: 23,
        tx_bps: 5,
    }
}

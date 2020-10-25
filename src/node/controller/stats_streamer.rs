use crate::node::stats::NodeStatsStreamFactory;
use crate::node::{NodeStats, NodeStatsObserver};
use act_zero::{call, send, Actor, ActorResult, Addr, AddrLike, Produces, WeakAddr};
use async_trait::async_trait;
use log::{info, trace};
use tokio::stream::{Stream, StreamExt};

pub struct StatsStreamer {
    hostname: String,
    stats_observer: Addr<dyn NodeStatsObserver>,
    node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
}

#[async_trait]
impl Actor for StatsStreamer {
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()> {
        info!("Started StatsStreamer {}", self.hostname);

        addr.send_fut({
            let stats_observer = self.stats_observer.clone();
            let hostname = self.hostname.clone();
            let stream_factory = self.node_stats_stream_factory.clone();

            async move { Self::poll_stream(stats_observer, hostname, stream_factory).await }
        });

        Produces::ok(())
    }
}

impl StatsStreamer {
    pub fn new(
        hostname: String,
        stats_observer: Addr<dyn NodeStatsObserver>,
        node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    ) -> Self {
        Self {
            hostname,
            stats_observer,
            node_stats_stream_factory,
        }
    }

    async fn poll_stream(
        addr: Addr<dyn NodeStatsObserver>,
        hostname: String,
        stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    ) {
        loop {
            info!("Opening StatsStream {}", hostname);
            let mut stats_stream = stats_stream_factory.create_stream(hostname.clone());

            while let Some(stats) = stats_stream.next().await {
                trace!("Received node stats from stream {} {:?}", hostname, stats);
                call!(addr.observe_node_stats(stats)).await;
            }
        }
    }
}

use crate::node::stats::NodeStatsStreamFactory;
use crate::node::{NodeStatsInfo, NodeStatsObserver};
use act_zero::{call, Actor, ActorResult, Addr, AddrLike, Produces, WeakAddr};
use async_trait::async_trait;
use tokio::stream::StreamExt;
use tracing::{info, trace, warn};

pub struct StatsStreamer {
    hostname: String,
    stats_observer: WeakAddr<dyn NodeStatsObserver>,
    node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
}

#[async_trait]
impl Actor for StatsStreamer {
    #[tracing::instrument(
        name = "StatsStreamer::started",
        skip(self, addr),
        fields(
            hostname = %self.hostname
        )
    )]
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
        stats_observer: WeakAddr<dyn NodeStatsObserver>,
        node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    ) -> Self {
        Self {
            hostname,
            stats_observer,
            node_stats_stream_factory,
        }
    }

    #[tracing::instrument(name = "StatsStreamer::poll_stream", skip(addr, stats_stream_factory))]
    async fn poll_stream(
        addr: WeakAddr<dyn NodeStatsObserver>,
        hostname: String,
        stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    ) {
        loop {
            info!("Opening StatsStream");
            let mut stats_stream = stats_stream_factory.create_stream(hostname.clone());

            while let Some(stats) = stats_stream.next().await {
                trace!("Received node stats from stream {:?}", stats);
                let publishing_result = call!(addr.observe_node_stats(NodeStatsInfo {
                    hostname: hostname.clone(),
                    stats,
                }))
                .await;

                if let Err(e) = publishing_result {
                    warn!("Failed to publish NodeStats {:?}", e)
                }
            }
        }
    }
}

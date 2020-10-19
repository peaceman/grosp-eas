use crate::node::{NodeStats, NodeStatsObserver};
use act_zero::{call, send, Actor, ActorResult, Addr, AddrLike, Produces, WeakAddr};
use async_trait::async_trait;
use log::info;
use tokio::stream::{Stream, StreamExt};

pub struct NodeController<NSS, NSSF>
where
    NSS: Stream<Item = NodeStats> + Send + Unpin + 'static,
    NSSF: Fn(String) -> NSS + Send + Sync + 'static,
{
    hostname: String,
    addr: WeakAddr<Self>,
    stats_observer: WeakAddr<dyn NodeStatsObserver>,
    node_stats_source_factory: NSSF,
}

#[async_trait]
impl<NSS, NSSF> Actor for NodeController<NSS, NSSF>
where
    NSS: Stream<Item = NodeStats> + Send + Unpin + 'static,
    NSSF: Fn(String) -> NSS + Send + Sync + 'static,
{
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started NodeController {}", self.hostname);

        self.addr = addr.downgrade();

        let weak_addr = addr.downgrade();
        let nss = (self.node_stats_source_factory)(self.hostname.clone());
        addr.send_fut(async move { Self::poll_stream(weak_addr, nss).await });

        Produces::ok(())
    }
}

impl<NSS, NSSF> NodeController<NSS, NSSF>
where
    NSS: Stream<Item = NodeStats> + Send + Unpin + 'static,
    NSSF: Fn(String) -> NSS + Send + Sync + 'static,
{
    pub fn new(
        hostname: String,
        stats_observer: WeakAddr<dyn NodeStatsObserver>,
        nss_factory: NSSF,
    ) -> Self {
        Self {
            hostname,
            stats_observer,
            node_stats_source_factory: nss_factory,
            addr: Default::default(),
        }
    }

    async fn stop(&mut self) -> ActorResult<()> {
        Produces::ok(())
    }

    async fn publish_stats(&self, stats: NodeStats) -> ActorResult<()> {
        info!("Publish stats {:?}", stats);

        send!(self.stats_observer.observe_node_stats(stats));

        Produces::ok(())
    }

    async fn poll_stream(addr: WeakAddr<Self>, mut stats_stream: NSS) {
        while let Some(stats) = stats_stream.next().await {
            send!(addr.publish_stats(stats));
        }
    }
}

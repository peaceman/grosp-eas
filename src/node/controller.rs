mod config;
mod state_machine;

use crate::cloud_provider::CloudProvider;
use crate::dns_provider::DnsProvider;
use crate::node::controller::state_machine::{NodeMachine, NodeMachineEvent};
use crate::node::{NodeStats, NodeStatsObserver};
use crate::node_discovery::{NodeDiscoveryData, NodeDiscoveryProvider};
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{call, send, Actor, ActorResult, Addr, AddrLike, Produces, WeakAddr};
use async_trait::async_trait;
use log::info;
use std::time::Duration;
use tokio::stream::{Stream, StreamExt};

use config::Config;

pub struct NodeController<NSS, NSSF>
where
    NSS: Stream<Item = NodeStats> + Send + Unpin + 'static,
    NSSF: Fn(String) -> NSS + Send + Sync + 'static,
{
    hostname: String,
    addr: WeakAddr<Self>,
    stats_observer: WeakAddr<dyn NodeStatsObserver>,
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_stats_source_factory: NSSF,
    node_machine_timer: Timer,
    node_machine: Option<NodeMachine>,
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

        self.node_machine_timer
            .set_interval_weak(self.addr.clone(), Duration::from_secs(1));

        // let weak_addr = addr.downgrade();
        // let nss = (self.node_stats_source_factory)(self.hostname.clone());
        // addr.send_fut(async move { Self::poll_stream(weak_addr, nss).await });

        Produces::ok(())
    }
}

#[async_trait]
impl<NSS, NSSF> Tick for NodeController<NSS, NSSF>
where
    NSS: Stream<Item = NodeStats> + Send + Unpin + 'static,
    NSSF: Fn(String) -> NSS + Send + Sync + 'static,
{
    async fn tick(&mut self) -> ActorResult<()> {
        if self.node_machine_timer.tick() {
            send!(self.addr.process_node_machine(None));
        }

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
        node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        nss_factory: NSSF,
    ) -> Self {
        Self {
            hostname: hostname.clone(),
            stats_observer,
            cloud_provider: cloud_provider.clone(),
            dns_provider: dns_provider.clone(),
            node_stats_source_factory: nss_factory,
            addr: Default::default(),
            node_machine_timer: Default::default(),
            node_machine: Some(NodeMachine::new(
                hostname,
                node_discovery_provider.clone(),
                cloud_provider.clone(),
                dns_provider.clone(),
                Config {
                    draining_time: Duration::from_secs(2 * 60),
                    provisioning_timeout: Duration::from_secs(10 * 60),
                },
            )),
        }
    }

    pub async fn provision_node(&mut self) {
        self.process_node_machine(Some(NodeMachineEvent::ProvisionNode))
            .await;
    }

    pub async fn discovered_node(&mut self, discovery_data: NodeDiscoveryData) {
        self.process_node_machine(Some(NodeMachineEvent::DiscoveredNode { discovery_data }))
            .await;
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

    async fn process_node_machine(&mut self, event: Option<NodeMachineEvent>) {
        info!("Process node machine {}", self.hostname);

        self.node_machine = Some(self.node_machine.take().unwrap().handle(event).await);
    }
}

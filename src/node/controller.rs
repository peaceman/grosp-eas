mod config;
mod state_machine;
mod stats_streamer;

use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::controller::state_machine::{Data, Draining, NodeMachine, NodeMachineEvent};
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryProvider};
use crate::node::{
    NodeDrainingCause, NodeState, NodeStateInfo, NodeStateObserver, NodeStats, NodeStatsObserver,
};
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{call, send, Actor, ActorResult, Addr, AddrLike, Produces, WeakAddr};
use async_trait::async_trait;
use log::info;
use std::time::Duration;
use tokio::stream::{Stream, StreamExt};

use crate::node::stats::NodeStatsStreamFactory;
use config::Config;
use stats_streamer::StatsStreamer;

pub struct NodeController {
    hostname: String,
    addr: WeakAddr<Self>,
    node_machine_timer: Timer,
    node_machine: Option<NodeMachine>,
    node_state_observer: WeakAddr<dyn NodeStateObserver>,
}

#[async_trait]
impl Actor for NodeController {
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started NodeController {}", self.hostname);

        self.addr = addr.downgrade();

        self.node_machine_timer
            .set_interval_weak(self.addr.clone(), Duration::from_secs(1));

        Produces::ok(())
    }
}

#[async_trait]
impl Tick for NodeController {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.node_machine_timer.tick() {
            send!(self.addr.process_node_machine(None));
        }

        Produces::ok(())
    }
}

impl Drop for NodeController {
    fn drop(&mut self) {
        info!("Drop NodeController {}", self.hostname);
    }
}

impl NodeController {
    pub fn new(
        hostname: String,
        node_stats_observer: WeakAddr<dyn NodeStatsObserver>,
        node_state_observer: WeakAddr<dyn NodeStateObserver>,
        node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    ) -> Self {
        Self {
            hostname: hostname.clone(),
            addr: Default::default(),
            node_machine_timer: Default::default(),
            node_state_observer,
            node_machine: Some(NodeMachine::new(
                hostname,
                node_discovery_provider,
                cloud_provider,
                dns_provider,
                node_stats_observer,
                node_stats_stream_factory,
                Config {
                    draining_time: Duration::from_secs(2 * 60),
                    provisioning_timeout: Duration::from_secs(10 * 60),
                    discovery_timeout: Duration::from_secs(10 * 60),
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

    pub async fn explored_node(&mut self, node_info: CloudNodeInfo) {
        self.process_node_machine(Some(NodeMachineEvent::ExploredNode { node_info }))
            .await;
    }

    pub async fn activate_node(&mut self) {
        self.process_node_machine(Some(NodeMachineEvent::ActivateNode))
            .await;
    }

    pub async fn deprovision_node(&mut self, cause: NodeDrainingCause) {
        self.process_node_machine(Some(NodeMachineEvent::DeprovisionNode { cause }))
            .await;
    }

    async fn stop(&mut self) -> ActorResult<()> {
        Produces::ok(())
    }

    async fn process_node_machine(&mut self, event: Option<NodeMachineEvent>) {
        info!("Process node machine {}", self.hostname);

        self.node_machine = Some(
            self.node_machine
                .take()
                .unwrap()
                .handle(event, self.node_state_observer.clone())
                .await,
        );
    }
}

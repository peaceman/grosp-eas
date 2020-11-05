mod config;
mod state_machine;
mod stats_streamer;

use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::controller::state_machine::{Data, Draining, NodeMachine, NodeMachineEvent};
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryProvider, NodeDiscoveryState};
use crate::node::{
    Node, NodeDrainingCause, NodeState, NodeStateInfo, NodeStateObserver, NodeStats,
    NodeStatsObserver,
};
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{call, send, Actor, ActorResult, Addr, AddrLike, Produces, WeakAddr};
use async_trait::async_trait;
use std::fmt;
use std::time::Duration;
use tokio::stream::{Stream, StreamExt};
use tracing::info;

use crate::node::stats::NodeStatsStreamFactory;
use config::Config;
use stats_streamer::StatsStreamer;

pub struct NodeController {
    node: Node,
    addr: WeakAddr<Self>,
    node_machine_timer: Timer,
    node_machine: Option<NodeMachine>,
    node_state_observer: WeakAddr<dyn NodeStateObserver>,
}

impl fmt::Display for NodeController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeController ({})", self.node)
    }
}

impl fmt::Debug for NodeController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[async_trait]
impl Actor for NodeController {
    #[tracing::instrument(
        name = "NodeController::started",
        skip(self, addr),
        fields(hostname = %self.node.hostname, group = %self.node.group)
    )]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

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
        info!("Drop NodeController {}", self.node);
    }
}

impl NodeController {
    pub fn new(
        node: Node,
        node_stats_observer: WeakAddr<dyn NodeStatsObserver>,
        node_state_observer: WeakAddr<dyn NodeStateObserver>,
        node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    ) -> Self {
        Self {
            node: node.clone(),
            addr: Default::default(),
            node_machine_timer: Default::default(),
            node_state_observer,
            node_machine: Some(NodeMachine::new(
                node,
                node_discovery_provider,
                cloud_provider,
                dns_provider,
                node_stats_observer,
                node_stats_stream_factory,
                Config {
                    draining_time: Duration::from_secs(1 * 60),
                    provisioning_timeout: Duration::from_secs(2 * 60),
                    discovery_timeout: Duration::from_secs(2 * 60),
                },
            )),
        }
    }

    #[tracing::instrument(
        name = "NodeController::provision_node",
        skip(self),
        fields(hostname = %self.node.hostname, group = %self.node.group)
    )]
    pub async fn provision_node(&mut self, target_state: NodeDiscoveryState) {
        self.process_node_machine(Some(NodeMachineEvent::ProvisionNode { target_state }))
            .await;
    }

    #[tracing::instrument(
        name = "NodeController::discovered_node",
        skip(self, discovery_data),
        fields(hostname = %self.node.hostname)
    )]
    pub async fn discovered_node(&mut self, discovery_data: NodeDiscoveryData) {
        self.process_node_machine(Some(NodeMachineEvent::DiscoveredNode { discovery_data }))
            .await;
    }

    #[tracing::instrument(
        name = "NodeController::explored_node",
        skip(self, node_info),
        fields(hostname = %self.node.hostname, group = %self.node.group)
    )]
    pub async fn explored_node(&mut self, node_info: CloudNodeInfo) {
        self.process_node_machine(Some(NodeMachineEvent::ExploredNode { node_info }))
            .await;
    }

    #[tracing::instrument(
        name = "NodeController::activate_node",
        skip(self),
        fields(hostname = %self.node.hostname, group = %self.node.group)
    )]
    pub async fn activate_node(&mut self) {
        self.process_node_machine(Some(NodeMachineEvent::ActivateNode))
            .await;
    }

    #[tracing::instrument(
        name = "NodeController::deprovision_node",
        skip(self),
        fields(hostname = %self.node.hostname, group = %self.node.group)
    )]
    pub async fn deprovision_node(&mut self, cause: NodeDrainingCause) {
        self.process_node_machine(Some(NodeMachineEvent::DeprovisionNode { cause }))
            .await;
    }

    #[tracing::instrument(
        name = "NodeController::process_node_machine",
        skip(self),
        fields(hostname = %self.node.hostname, group = %self.node.group)
    )]
    async fn process_node_machine(&mut self, event: Option<NodeMachineEvent>) {
        self.node_machine = Some(
            self.node_machine
                .take()
                .unwrap()
                .handle(event, self.node_state_observer.clone())
                .await,
        );
    }
}

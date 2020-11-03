use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::discovery::{
    NodeDiscoveryData, NodeDiscoveryObserver, NodeDiscoveryProvider, NodeDiscoveryState,
};
use crate::node::exploration::NodeExplorationObserver;
use crate::node::stats::NodeStatsStreamFactory;
use crate::node::{
    Node, NodeController, NodeDrainingCause, NodeState, NodeStateInfo, NodeStateObserver,
    NodeStats, NodeStatsInfo, NodeStatsObserver,
};
use crate::node_groups::NodeGroup;
use act_zero::runtimes::tokio::{spawn_actor, Timer};
use act_zero::timer::Tick;
use act_zero::{send, upcast, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;
use tracing::{info, trace};

pub struct NodeGroupScaler {
    node_group: NodeGroup,
    timer: Timer,
    addr: WeakAddr<Self>,
    nodes: HashMap<String, ScalingNode>,
    node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
}

struct ScalingNode {
    controller: Addr<NodeController>,
    last_stats: Option<NodeStats>,
    state: NodeState,
}

impl NodeGroupScaler {
    pub fn new(
        node_group: NodeGroup,
        node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    ) -> Self {
        NodeGroupScaler {
            node_group,
            timer: Default::default(),
            addr: Default::default(),
            nodes: Default::default(),
            node_discovery_provider,
            cloud_provider,
            dns_provider,
            node_stats_stream_factory,
        }
    }
}

impl fmt::Display for NodeGroupScaler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeGroupScaler {}", self.node_group.name)
    }
}

impl fmt::Debug for NodeGroupScaler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[async_trait]
impl Actor for NodeGroupScaler {
    #[tracing::instrument(
        name = "NodeGroupScaler::started",
        skip(self, addr),
        fields(
            group = %self.node_group.name,
        )
    )]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

        self.addr = addr.downgrade();

        self.timer
            .set_interval_weak(self.addr.clone(), Duration::from_secs(1));

        Produces::ok(())
    }
}

#[async_trait]
impl Tick for NodeGroupScaler {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.remove_deprovisioned_nodes());

            if self.node_group.config.is_some() {
                send!(self.addr.scale());
            }
        }

        Produces::ok(())
    }
}

impl Drop for NodeGroupScaler {
    fn drop(&mut self) {
        info!("Drop {}", self);
    }
}

#[async_trait]
impl NodeDiscoveryObserver for NodeGroupScaler {
    #[tracing::instrument(
        name = "NodeGroupScaler::observe_node_discovery",
        skip(self, data),
        fields(
            group = %self.node_group.name,
        )
    )]
    async fn observe_node_discovery(&mut self, data: NodeDiscoveryData) {
        trace!(
            hostname = data.hostname.as_str(),
            state = format!("{:?}", data.state).as_str()
        );

        if !self.nodes.contains_key(&data.hostname) {
            self.nodes.insert(
                data.hostname.clone(),
                self.create_scaling_node(&data.hostname),
            );
        }

        let node = self.nodes.get(&data.hostname).unwrap();
        send!(node.controller.discovered_node(data));
    }
}

#[async_trait]
impl NodeExplorationObserver for NodeGroupScaler {
    #[tracing::instrument(
        name = "NodeGroupScaler::observe_node_exploration",
        skip(self, node_info),
        fields(
            group = %self.node_group.name,
        )
    )]
    async fn observe_node_exploration(&mut self, node_info: CloudNodeInfo) {
        trace!(
            hostname = node_info.hostname.as_str(),
            identifier = node_info.identifier.as_str(),
        );

        if !self.nodes.contains_key(&node_info.hostname) {
            let node = self.create_scaling_node(&node_info.hostname);

            self.nodes.insert(node_info.hostname.clone(), node);
        }

        let node = self.nodes.get(&node_info.hostname).unwrap();
        send!(node.controller.explored_node(node_info));
    }
}

#[async_trait]
impl NodeStateObserver for NodeGroupScaler {
    #[tracing::instrument(
        name = "NodeGroupScaler::observe_node_state",
        skip(self, state_info),
        fields(group = %self.node_group.name)
    )]
    async fn observe_node_state(&mut self, state_info: NodeStateInfo) {
        trace!(
            hostname = state_info.hostname.as_str(),
            state = format!("{:?}", state_info.state).as_str()
        );

        if let Some(scaling_node) = self.nodes.get_mut(&state_info.hostname) {
            scaling_node.state = state_info.state;
        }
    }
}

#[async_trait]
impl NodeStatsObserver for NodeGroupScaler {
    #[tracing::instrument(
        name = "NodeGroupScaler::observe_node_stats",
        skip(self, stats_info),
        fields(
            group = %self.node_group.name,
            hostname = %stats_info.hostname
        )
    )]
    async fn observe_node_stats(&mut self, stats_info: NodeStatsInfo) {
        trace!(stats = format!("{:?}", stats_info.stats).as_str());

        if let Some(scaling_node) = self.nodes.get_mut(&stats_info.hostname) {
            scaling_node.last_stats = Some(stats_info.stats);
        }
    }
}

impl NodeGroupScaler {
    #[tracing::instrument(
        name = "NodeGroupScaler::terminate",
        skip(self),
        fields(
            group = %self.node_group.name
        )
    )]
    pub async fn terminate(&mut self) -> ActorResult<()> {
        for scaling_node in self.nodes.values() {
            send!(scaling_node
                .controller
                .deprovision_node(NodeDrainingCause::Termination));
        }

        trace!(remaining_nodes = self.nodes.len());
        if self.nodes.is_empty() {
            Err(format!("Terminated all nodes {}", self).into())
        } else {
            Produces::ok(())
        }
    }

    #[tracing::instrument(
        name = "NodeGroupScaler::scale",
        skip(self),
        fields(
            group = %self.node_group.name
        )
    )]
    async fn scale(&mut self) -> ActorResult<()> {
        let bandwidth_usage = self.calculate_bandwidth_usage();
        info!(bandwidth_usage);

        Produces::ok(())
    }

    fn calculate_bandwidth_usage(&self) -> u8 {
        #[derive(Default, Debug)]
        struct ValueAcc {
            count: u64,
            bandwidth: u64,
        }

        let result = self
            .nodes
            .values()
            .filter(|n| n.state.is_active())
            .filter_map(|n| n.last_stats.as_ref())
            .fold(ValueAcc::default(), |mut acc, ns| {
                acc.count += 1;
                acc.bandwidth += ns.tx_bps;

                acc
            });

        let node_group_config = self.node_group.config.as_ref().unwrap();
        let max_capacity = result.count * node_group_config.node_bandwidth_capacity.tx_bps;

        ((result.bandwidth as f64 / max_capacity as f64) * 100 as f64) as u8
    }

    #[tracing::instrument(
        name = "NodeGroupScaler::remove_deprovisioned_nodes",
        skip(self),
        fields(
            group = %self.node_group.name
        )
    )]
    async fn remove_deprovisioned_nodes(&mut self) {
        self.nodes
            .retain(|_, scaling_node| match scaling_node.state {
                NodeState::Deprovisioned => false,
                _ => true,
            });
    }

    #[tracing::instrument(
        name = "NodeGroupScaler::create_scaling_node",
        skip(self, hostname),
        fields(
            group = %self.node_group.name,
            hostname = %hostname.as_ref()
        )
    )]
    fn create_scaling_node(&self, hostname: impl AsRef<str>) -> ScalingNode {
        let node_controller = NodeController::new(
            Node {
                hostname: hostname.as_ref().clone().into(),
                group: self.node_group.name.clone(),
            },
            upcast!(self.addr.clone()),
            upcast!(self.addr.clone()),
            self.node_discovery_provider.clone(),
            self.cloud_provider.clone(),
            self.dns_provider.clone(),
            self.node_stats_stream_factory.clone(),
        );

        ScalingNode {
            state: NodeState::Unready,
            last_stats: None,
            controller: spawn_actor(node_controller),
        }
    }
}

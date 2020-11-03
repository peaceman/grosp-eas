use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::discovery::{
    NodeDiscoveryData, NodeDiscoveryObserver, NodeDiscoveryProvider, NodeDiscoveryState,
};
use crate::node::exploration::NodeExplorationObserver;
use crate::node::stats::NodeStatsStreamFactory;
use crate::node::{
    HostnameGenerator, Node, NodeController, NodeDrainingCause, NodeState, NodeStateInfo,
    NodeStateObserver, NodeStats, NodeStatsInfo, NodeStatsObserver,
};
use crate::node_groups::{Config, NodeGroup};
use act_zero::runtimes::tokio::{spawn_actor, Timer};
use act_zero::timer::Tick;
use act_zero::{send, upcast, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, trace};

pub struct NodeGroupScaler {
    node_group: NodeGroup,
    timer: Timer,
    addr: WeakAddr<Self>,
    nodes: HashMap<String, ScalingNode>,
    scale_lock: Option<ScaleLock>,
    node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    hostname_generator: Arc<dyn HostnameGenerator>,
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
        hostname_generator: Arc<dyn HostnameGenerator>,
    ) -> Self {
        NodeGroupScaler {
            node_group,
            timer: Default::default(),
            addr: Default::default(),
            nodes: Default::default(),
            scale_lock: None,
            node_discovery_provider,
            cloud_provider,
            dns_provider,
            node_stats_stream_factory,
            hostname_generator,
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
    async fn scale(&mut self) {
        if self.node_group.config.is_none() {
            return;
        }

        if self.scale_lock.is_some() {
            self.check_scale_lock();
        } else {
            let thresholds = self
                .node_group
                .config
                .as_ref()
                .map(|c| c.bandwidth_thresholds)
                .unwrap();

            match self.calculate_bandwidth_usage_percent() {
                // scale up
                x if x > thresholds.scale_up_percent => {
                    info!(bandwidth_usage_percent = x, "Trigger ScaleUp");
                    self.scale_lock = Some(self.scale_up().await);
                }
                // scale down
                x if x < thresholds.scale_down_percent => {
                    info!(bandwidth_usage_percent = x, "Trigger ScaleDown");
                    self.scale_lock = self.scale_down().await;
                }
                // do nothing
                x => info!(
                    bandwidth_usage_percent = x,
                    "Current bandwidth usage doesn't exceed any thresholds, do nothing"
                ),
            }
        }
    }

    #[tracing::instrument(name = "NodeGroupScaler::check_scale_lock", skip(self))]
    fn check_scale_lock(&mut self) {
        // todo scale lock expiry
        let scale_lock = self.scale_lock.as_ref().unwrap();

        info!(scale_lock = format!("{:?}", scale_lock).as_str());

        let release_scale_lock = match &scale_lock.expectation {
            ScaleLockExpectation::Gone => !self.nodes.contains_key(&scale_lock.hostname),
            ScaleLockExpectation::State(expected_node_state) => {
                match self.nodes.get(&scale_lock.hostname) {
                    Some(node) if &node.state == expected_node_state => true,
                    _ => false,
                }
            }
        };

        if release_scale_lock {
            self.scale_lock = None;
        }
    }

    async fn scale_up(&mut self) -> ScaleLock {
        // try re-activating nodes from draining state
        if let Some(scale_lock) = self.reactivate_draining_node().await {
            return scale_lock;
        }

        // try activating ready nodes
        if let Some(scale_lock) = self.activate_ready_node().await {
            return scale_lock;
        }

        // provision new node
        // todo check max nodes
        self.provision_new_node().await
    }

    async fn reactivate_draining_node(&mut self) -> Option<ScaleLock> {
        let reactivatable = self
            .nodes
            .iter()
            .find(|(k, v)| v.state.is_draining(NodeDrainingCause::Scaling));

        if reactivatable.is_none() {
            return None;
        }

        let (hostname, node) = reactivatable.unwrap();

        info!(%hostname, "Found re-activatable draining node");
        send!(node.controller.activate_node());

        Some(ScaleLock::new(
            hostname.clone(),
            ScaleLockExpectation::State(NodeState::Active),
        ))
    }

    async fn activate_ready_node(&mut self) -> Option<ScaleLock> {
        let reactivatable = self.nodes.iter().find(|(k, v)| v.state.is_ready());

        if reactivatable.is_none() {
            return None;
        }

        let (hostname, node) = reactivatable.unwrap();

        info!(%hostname, "Found activatable ready node");
        send!(node.controller.activate_node());

        Some(ScaleLock::new(
            hostname.clone(),
            ScaleLockExpectation::State(NodeState::Active),
        ))
    }

    async fn provision_new_node(&mut self) -> ScaleLock {
        let hostname = self
            .hostname_generator
            .generate_hostname(self.node_group.name.as_ref());

        let node = self.create_scaling_node(&hostname);

        self.nodes.insert(hostname.clone(), node);

        ScaleLock::new(hostname, ScaleLockExpectation::State(NodeState::Ready))
    }

    async fn scale_down(&mut self) -> Option<ScaleLock> {
        let min_nodes = self.node_group.config.as_ref().unwrap().min_nodes;
        let min_nodes = min_nodes.map_or(1, |v| max(v, 1));

        let mut active_nodes_info = self
            .nodes
            .iter()
            .filter(|(k, v)| v.state.is_active())
            .collect::<Vec<(&String, &ScalingNode)>>();

        if min_nodes >= active_nodes_info.len() as u64 {
            info!(
                min_nodes,
                current_active_nodes = active_nodes_info.len(),
                "Cancel scale down"
            );
            None
        } else {
            let (hostname, node) = active_nodes_info.pop().unwrap();
            info!(%hostname, "De-provision node");
            send!(node.controller.deprovision_node(NodeDrainingCause::Scaling));

            Some(ScaleLock::new(hostname.into(), ScaleLockExpectation::Gone))
        }
    }

    fn calculate_bandwidth_usage_percent(&self) -> u8 {
        #[derive(Default, Debug)]
        struct ValueAcc {
            count: u64,
            bandwidth: u64,
        }

        let result = self.nodes.values().filter(|n| n.state.is_active()).fold(
            ValueAcc::default(),
            |mut acc, n| {
                acc.count += 1;
                acc.bandwidth += n.last_stats.as_ref().map_or(0, |ns| ns.tx_bps);

                acc
            },
        );

        let node_group_config = self.node_group.config.as_ref().unwrap();
        let max_capacity = result.count * node_group_config.node_bandwidth_capacity.tx_bps;

        match max_capacity {
            0 => 0,
            max_capacity => ((result.bandwidth as f64 / max_capacity as f64) * 100 as f64) as u8,
        }
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
        name = "NodeGroupScaler:update_node_group_config",
        skip(self),
        fields(
            group = %self.node_group.name
        )
    )]
    pub async fn update_node_group_config(&mut self, node_group_config: Option<Config>) {
        self.node_group.config = node_group_config;
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

#[derive(Debug)]
struct ScaleLock {
    expectation: ScaleLockExpectation,
    hostname: String,
    created_at: Instant,
}

impl ScaleLock {
    fn new(hostname: String, expectation: ScaleLockExpectation) -> Self {
        Self {
            hostname,
            expectation,
            created_at: Instant::now(),
        }
    }
}

#[derive(Debug)]
enum ScaleLockExpectation {
    State(NodeState),
    Gone,
}

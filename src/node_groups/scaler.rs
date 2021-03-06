use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::discovery::{
    NodeDiscoveryData, NodeDiscoveryObserver, NodeDiscoveryProvider, NodeDiscoveryState,
};
use crate::node::exploration::NodeExplorationObserver;
use crate::node::stats::NodeStatsStreamFactory;
use crate::node::{
    HostnameGenerator, Node, NodeController, NodeControllerProviders, NodeDrainingCause, NodeState,
    NodeStateInfo, NodeStateObserver, NodeStats, NodeStatsInfo, NodeStatsObserver,
};
use crate::node_groups::{Config, NodeGroup};
use crate::{actor, AppConfig};
use act_zero::runtimes::tokio::{spawn_actor, Timer};
use act_zero::timer::Tick;
use act_zero::{send, upcast, Actor, ActorError, ActorResult, Addr, Produces, WeakAddr};
use anyhow::anyhow;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, trace};

pub struct NodeGroupScaler {
    node_group: NodeGroup,
    timer: Timer,
    addr: WeakAddr<Self>,
    nodes: HashMap<String, ScalingNode>,
    scale_locks: Option<Vec<ScaleLock>>,
    node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    hostname_generator: Arc<dyn HostnameGenerator>,
    scale_locks_spare: SpareScaleLocks,
    config: AppConfig,
    is_terminating: bool,
    started_at: Instant,
}

#[derive(Default)]
struct SpareScaleLocks {
    up: HashMap<String, ScaleLock>,
    down: HashMap<String, ScaleLock>,
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
        config: AppConfig,
    ) -> Self {
        NodeGroupScaler {
            node_group,
            timer: Default::default(),
            addr: Default::default(),
            nodes: Default::default(),
            scale_locks: None,
            scale_locks_spare: Default::default(),
            is_terminating: false,
            started_at: Instant::now(),
            node_discovery_provider,
            cloud_provider,
            dns_provider,
            node_stats_stream_factory,
            hostname_generator,
            config,
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

    async fn error(&mut self, error: ActorError) -> bool {
        actor::handle_error(error)
    }
}

#[async_trait]
impl Tick for NodeGroupScaler {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.remove_deprovisioned_nodes());

            if self.should_scale() {
                send!(self.addr.check_scale_locks());
                send!(self.addr.scale_spare());
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
    fn should_scale(&self) -> bool {
        !self.is_terminating
            && self.node_group.config.is_some()
            && Instant::now().duration_since(self.started_at)
                > self.config.node_group_scaler.startup_cooldown
    }

    #[tracing::instrument(
        name = "NodeGroupScaler::terminate",
        skip(self),
        fields(
            group = %self.node_group.name
        )
    )]
    pub async fn terminate(&mut self) -> ActorResult<()> {
        self.is_terminating = true;

        for scaling_node in self.nodes.values() {
            send!(scaling_node
                .controller
                .deprovision_node(NodeDrainingCause::Termination));
        }

        trace!(remaining_nodes = self.nodes.len());
        if self.nodes.is_empty() {
            let err: actor::Error =
                actor::ErrorKind::Fatal(anyhow!("Terminated all nodes {}", self)).into();

            Err(err.into())
        } else {
            Produces::ok(())
        }
    }

    #[tracing::instrument(
        name = "NodeGroupScaler::scale_spare"
        skip(self),
        fields(group = %self.node_group.name)
    )]
    async fn scale_spare(&mut self) {
        if self.node_group.config.is_none() {
            return;
        }

        self.check_spare_scale_locks();

        let mut ready_nodes = HashSet::new();

        // add currently existing ready nodes
        self.nodes
            .iter()
            .filter(|(_h, sn)| sn.state.is_ready())
            .for_each(|(h, _sn)| {
                ready_nodes.insert(h);
            });

        // add nodes that are expected to be ready soon
        self.scale_locks_spare.up.keys().for_each(|h| {
            ready_nodes.insert(h);
        });

        // remove nodes that are expected to be gone
        self.scale_locks_spare.down.keys().for_each(|h| {
            ready_nodes.remove(h);
        });

        let projected_ready_nodes = ready_nodes.len() as u32;
        let node_change = self.determine_necessary_spare_node_change(projected_ready_nodes);

        info!(projected_ready_nodes, node_change);

        if node_change.is_positive() {
            self.provision_spare_nodes(node_change as u32);
        } else {
            self.deprovision_spare_nodes(node_change.abs() as u32);
        }
    }

    fn check_spare_scale_locks(&mut self) {
        // check scale down locks
        self.scale_locks_spare.down.retain({
            let nodes = &self.nodes;
            move |_hostname, lock| !is_releasable_scale_lock(nodes, lock)
        });

        // check scale up locks
        self.scale_locks_spare.up.retain({
            let nodes = &self.nodes;
            move |_hostname, lock| !is_releasable_scale_lock(nodes, lock)
        });
    }

    fn determine_necessary_spare_node_change(&self, ready_nodes: u32) -> i32 {
        let (min_spare_nodes, mut max_spare_nodes) = {
            let config = self.node_group.config.as_ref().unwrap();

            (config.min_spare_nodes, config.max_spare_nodes)
        };

        // ensure max spare nodes is always equal or greater than min spare nodes
        if let (Some(min), Some(max)) = (min_spare_nodes, max_spare_nodes) {
            if min > max {
                max_spare_nodes = Some(min);
            }
        }

        let node_change = max_spare_nodes
            .map(|max| {
                if ready_nodes > max {
                    max as i32 - ready_nodes as i32
                } else {
                    0
                }
            })
            .unwrap_or(0i32);

        node_change
            + min_spare_nodes
                .map(|min| {
                    if ready_nodes < min {
                        min - ready_nodes
                    } else {
                        0
                    }
                })
                .map(|v| v as i32)
                .unwrap_or(0i32)
    }

    fn release_spare_lock(&mut self, hostname: &str) {
        let up = self.scale_locks_spare.up.remove(hostname);
        let down = self.scale_locks_spare.down.remove(hostname);

        if up.or(down).is_some() {
            info!(hostname, "Release spare lock");
        }
    }

    #[tracing::instrument(
        name = "NodeGroupScaler::provision_spare_nodes"
        skip(self),
        fields(group = %self.node_group.name)
    )]
    fn provision_spare_nodes(&mut self, amount: u32) {
        for _i in 0..amount {
            match self.try_provision_new_node(NodeDiscoveryState::Ready) {
                Some(scale_lock) => {
                    self.scale_locks_spare
                        .up
                        .insert(scale_lock.hostname.clone(), scale_lock);
                }
                None => break,
            }
        }
    }

    #[tracing::instrument(
        name = "NodeGroupScaler::deprovision_spare_nodes"
        skip(self),
        fields(group = %self.node_group.name)
    )]
    fn deprovision_spare_nodes(&mut self, amount: u32) {
        let ready_nodes = self
            .nodes
            .iter()
            .filter(|(_h, n)| n.state.is_ready())
            .filter({
                let locks = &self.scale_locks_spare;
                move |(h, _n)| !locks.up.contains_key(*h) && !locks.down.contains_key(*h)
            })
            .take(amount as usize);

        let mut locks = Vec::with_capacity(amount as usize);
        for (hostname, node) in ready_nodes {
            let scale_lock =
                self.deprovision_node(hostname.as_str(), node, NodeDrainingCause::Termination);

            locks.push((hostname.clone(), scale_lock));
        }

        for (hostname, lock) in locks.into_iter() {
            self.scale_locks_spare.down.insert(hostname, lock);
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

        if self.scale_locks.is_some() {
            return;
        }

        self.scale_locks = None
            .or_else(|| self.scale_min_active_nodes())
            .or_else(|| self.scale_bandwidth_nodes());
    }

    fn scale_min_active_nodes(&mut self) -> Option<Vec<ScaleLock>> {
        // check min active nodes
        let min_active_nodes = get_min_active_nodes(&self.node_group);
        let cur_active_nodes = self.get_active_node_count();

        if cur_active_nodes >= min_active_nodes {
            None
        } else {
            let missing_nodes = min_active_nodes - cur_active_nodes;
            info!(
                missing_nodes,
                "Trigger provisioning of new active nodes to reach the minimum"
            );

            let scale_locks = (0..missing_nodes)
                .filter_map(|_| self.try_provision_new_node(NodeDiscoveryState::Active))
                .collect();

            Some(scale_locks)
        }
    }

    fn get_active_node_count(&self) -> u32 {
        self.nodes.values().filter(|n| n.state.is_active()).count() as u32
    }

    fn scale_bandwidth_nodes(&mut self) -> Option<Vec<ScaleLock>> {
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
                self.scale_up()
            }
            // scale down
            x if x < thresholds.scale_down_percent => {
                info!(bandwidth_usage_percent = x, "Trigger ScaleDown");
                self.scale_down()
            }
            // do nothing
            x => {
                info!(
                    bandwidth_usage_percent = x,
                    "Current bandwidth usage doesn't exceed any thresholds, do nothing"
                );
                None
            }
        }
    }

    #[tracing::instrument(name = "NodeGroupScaler::check_scale_locks", skip(self))]
    async fn check_scale_locks(&mut self) {
        self.scale_locks = match self.scale_locks.take() {
            Some(mut scale_locks) => {
                scale_locks.retain(|scale_lock| !is_releasable_scale_lock(&self.nodes, scale_lock));

                if scale_locks.is_empty() {
                    None
                } else {
                    Some(scale_locks)
                }
            }
            None => None,
        };
    }

    fn scale_up(&mut self) -> Option<Vec<ScaleLock>> {
        None
            // try re-activating nodes from draining state
            .or_else(|| self.try_reactivate_draining_node())
            // try activating ready nodes
            .or_else(|| self.try_activate_ready_node())
            // provision new node
            .or_else(|| {
                self.try_provision_new_node(NodeDiscoveryState::Active)
                    .map(|sl| vec![sl])
            })
    }

    fn try_reactivate_draining_node(&mut self) -> Option<Vec<ScaleLock>> {
        let (hostname, node) = self
            .nodes
            .iter()
            .find(|(_k, v)| v.state.is_draining(NodeDrainingCause::Scaling))?;

        info!(%hostname, "Found re-activatable draining node");
        send!(node.controller.activate_node());

        Some(vec![ScaleLock::new(
            hostname.clone(),
            ScaleLockExpectation::State(NodeState::Active),
            ScaleLockCooldowns::new(Some(future_instant(15)), future_instant(30)),
        )])
    }

    fn try_activate_ready_node(&mut self) -> Option<Vec<ScaleLock>> {
        let (hostname, node) = self.nodes.iter().find(|(_k, v)| v.state.is_ready())?;

        info!(%hostname, "Found activatable ready node");
        send!(node.controller.activate_node());

        let hostname = hostname.clone();

        self.release_spare_lock(hostname.as_str());

        Some(vec![ScaleLock::new(
            hostname,
            ScaleLockExpectation::State(NodeState::Active),
            ScaleLockCooldowns::new(Some(future_instant(15)), future_instant(30)),
        )])
    }

    fn try_provision_new_node(&mut self, target_state: NodeDiscoveryState) -> Option<ScaleLock> {
        let current_nodes = self.nodes.len() as u32;
        let reached_node_limit = self
            .node_group
            .config
            .as_ref()
            .unwrap()
            .max_nodes
            .map(|max_nodes| current_nodes >= max_nodes);

        match reached_node_limit {
            Some(true) => {
                info!(%current_nodes, "Reached node limit, cancel node provisioning");
                None
            }
            Some(false) | None => {
                let hostname = self.provision_new_node(target_state.clone());

                Some(ScaleLock::new(
                    hostname,
                    ScaleLockExpectation::State(target_state.into()),
                    ScaleLockCooldowns::new(
                        None,
                        future_instant(self.config.node_group_scaler.scale_lock_timeout_s),
                    ),
                ))
            }
        }
    }

    fn provision_new_node(&mut self, target_state: NodeDiscoveryState) -> String {
        let hostname = self
            .hostname_generator
            .generate_hostname(self.node_group.name.as_ref());

        info!(%hostname, "Provision node");

        let node = self.create_scaling_node(&hostname);
        send!(node.controller.provision_node(target_state));

        self.nodes.insert(hostname.clone(), node);

        hostname
    }

    fn scale_down(&mut self) -> Option<Vec<ScaleLock>> {
        let min_active_nodes = get_min_active_nodes(&self.node_group);

        let mut active_nodes_info = self
            .nodes
            .iter()
            .filter(|(_k, v)| v.state.is_active())
            .collect::<Vec<(&String, &ScalingNode)>>();

        if min_active_nodes >= active_nodes_info.len() as u32 {
            info!(
                min_active_nodes,
                current_active_nodes = active_nodes_info.len(),
                "Cancel scale down"
            );
            None
        } else {
            let (hostname, node) = active_nodes_info.pop().unwrap();
            Some(vec![self.deprovision_node(
                hostname.as_str(),
                node,
                NodeDrainingCause::Scaling,
            )])
        }
    }

    fn deprovision_node(
        &self,
        hostname: &str,
        node: &ScalingNode,
        cause: NodeDrainingCause,
    ) -> ScaleLock {
        info!(%hostname, cause = format!("{:?}", cause).as_str(), "De-provision node");
        send!(node.controller.deprovision_node(cause));

        match cause {
            NodeDrainingCause::Scaling => ScaleLock::new(
                hostname.into(),
                ScaleLockExpectation::State(NodeState::Ready),
                ScaleLockCooldowns::new(Some(future_instant(15)), future_instant(30)),
            ),
            _ => ScaleLock::new(
                hostname.into(),
                ScaleLockExpectation::Gone,
                ScaleLockCooldowns::new(
                    None,
                    future_instant(self.config.node_group_scaler.scale_lock_timeout_s),
                ),
            ),
        }
    }

    fn calculate_bandwidth_usage_percent(&self) -> u8 {
        #[derive(Default, Debug)]
        struct ValueAcc {
            count: u64,
            bandwidth: u64,
        }

        let node_group_config = self.node_group.config.as_ref().unwrap();
        let tx_bps_node_capacity = node_group_config.node_bandwidth_capacity.tx_bps;

        let result = self.nodes.values().filter(|n| n.state.is_active()).fold(
            ValueAcc::default(),
            |mut acc, n| {
                acc.count += 1;
                acc.bandwidth += n
                    .last_stats
                    .as_ref()
                    .map_or(tx_bps_node_capacity / 2, |ns| ns.tx_bps);

                acc
            },
        );

        let max_capacity = result.count * tx_bps_node_capacity;

        match max_capacity {
            0 => 0,
            max_capacity => ((result.bandwidth as f64 / max_capacity as f64) * 100f64) as u8,
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
            .retain(|_, scaling_node| !matches!(scaling_node.state, NodeState::Deprovisioned));
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
        let hostname = hostname.as_ref().to_string();

        let node_controller = NodeController::new(
            Node {
                hostname,
                group: self.node_group.name.clone(),
            },
            upcast!(self.addr.clone()),
            upcast!(self.addr.clone()),
            NodeControllerProviders {
                node_discovery_provider: self.node_discovery_provider.clone(),
                cloud_provider: self.cloud_provider.clone(),
                dns_provider: self.dns_provider.clone(),
            },
            self.node_stats_stream_factory.clone(),
            Arc::clone(&self.config),
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
    cooldowns: ScaleLockCooldowns,
}

impl ScaleLock {
    fn new(
        hostname: String,
        expectation: ScaleLockExpectation,
        cooldowns: ScaleLockCooldowns,
    ) -> Self {
        Self {
            hostname,
            expectation,
            cooldowns,
        }
    }
}

#[derive(Debug)]
enum ScaleLockExpectation {
    State(NodeState),
    Gone,
}

#[derive(Debug)]
struct ScaleLockCooldowns {
    min: Option<Instant>,
    max: Instant,
}

impl ScaleLockCooldowns {
    fn new(min: Option<Instant>, max: Instant) -> Self {
        Self { min, max }
    }

    fn can_release(&self) -> bool {
        self.min.map(|min| Instant::now() >= min).unwrap_or(true)
    }

    fn must_release(&self) -> bool {
        Instant::now() >= self.max
    }
}

fn get_min_active_nodes(node_group: &NodeGroup) -> u32 {
    node_group
        .config
        .as_ref()
        .unwrap()
        .min_active_nodes
        .unwrap_or(1)
}

#[tracing::instrument(
    skip(nodes, scale_lock),
    fields(hostname = scale_lock.hostname.as_str(), expectation = format!("{:?}", scale_lock.expectation).as_str(), cooldowns = format!("{:?}", scale_lock.cooldowns).as_str())
)]
fn is_releasable_scale_lock(nodes: &HashMap<String, ScalingNode>, scale_lock: &ScaleLock) -> bool {
    if !scale_lock.cooldowns.can_release() {
        info!("Minimum cooldown period was not reached");
        return false;
    }

    if scale_lock.cooldowns.must_release() {
        info!("Maximum cooldown period was reached");
        return true;
    }

    let fulfilled_expectation = match &scale_lock.expectation {
        ScaleLockExpectation::Gone => !nodes.contains_key(&scale_lock.hostname),
        ScaleLockExpectation::State(expected_node_state) => {
            matches!(nodes.get(&scale_lock.hostname), Some(node) if &node.state == expected_node_state)
        }
    };

    if fulfilled_expectation {
        trace!(
            hostname = scale_lock.hostname.as_str(),
            expectation = format!("{:?}", scale_lock.expectation).as_str(),
            "Release ScaleLock"
        );
    }

    fulfilled_expectation
}

fn future_instant(secs: u64) -> Instant {
    Instant::now() + Duration::from_secs(secs)
}

use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::discovery::{
    NodeDiscoveryData, NodeDiscoveryObserver, NodeDiscoveryProvider, NodeDiscoveryState,
};
use crate::node::exploration::NodeExplorationObserver;
use crate::node::stats::NodeStatsStreamFactory;
use crate::node::{NodeController, NodeState, NodeStats};
use crate::node_groups::NodeGroup;
use act_zero::runtimes::tokio::{spawn_actor, Timer};
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use log::info;
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

pub struct NodeGroupScaler {
    node_group: NodeGroup,
    should_terminate: bool,
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
            should_terminate: false,
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

#[async_trait]
impl Actor for NodeGroupScaler {
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started {}", self);

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
            send!(self.addr.scale());
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
    async fn observe_node_discovery(&mut self, data: NodeDiscoveryData) {
        info!(
            "Observed node discovery ({}) {:?}",
            self.node_group.name, data
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
    async fn observe_node_exploration(&mut self, node_info: CloudNodeInfo) {
        info!(
            "Observed node exploration ({}) {:?}",
            self.node_group.name, node_info
        );

        if !self.nodes.contains_key(&node_info.hostname) {
            let node = self.create_scaling_node(&node_info.hostname);

            self.nodes.insert(node_info.hostname.clone(), node);
        }

        let node = self.nodes.get(&node_info.hostname).unwrap();
        send!(node.controller.explored_node(node_info));
    }
}

impl NodeGroupScaler {
    pub async fn terminate(&mut self) -> ActorResult<()> {
        self.should_terminate = true;

        Err("Terminate".into())
    }

    async fn scale(&mut self) -> ActorResult<()> {
        info!("Scale {}", self.node_group.name);

        Produces::ok(())
    }

    fn create_scaling_node(&self, hostname: impl AsRef<str>) -> ScalingNode {
        info!(
            "Create scaling node ({}) {}",
            self.node_group.name,
            hostname.as_ref()
        );

        let node_controller = NodeController::new(
            hostname.as_ref().into(),
            Default::default(), // todo add real node stats observer
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

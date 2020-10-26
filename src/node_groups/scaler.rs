use crate::cloud_provider::CloudNodeInfo;
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryObserver};
use crate::node::exploration::NodeExplorationObserver;
use crate::node_groups::NodeGroup;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use log::info;
use std::fmt;
use std::time::Duration;

pub struct NodeGroupScaler {
    node_group: NodeGroup,
    should_terminate: bool,
    timer: Timer,
    addr: WeakAddr<Self>,
}

impl NodeGroupScaler {
    pub fn new(node_group: NodeGroup) -> Self {
        NodeGroupScaler {
            node_group,
            should_terminate: false,
            timer: Default::default(),
            addr: Default::default(),
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
        unimplemented!()
    }
}

#[async_trait]
impl NodeExplorationObserver for NodeGroupScaler {
    async fn observe_node_exploration(&mut self, node_info: CloudNodeInfo) {
        unimplemented!()
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
}

mod state_machine;

use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryObserver, NodeDiscoveryProvider};
use crate::node::exploration::NodeExplorationObserver;
use crate::node::stats::NodeStatsStreamFactory;
use crate::node_groups::controller::state_machine::NodeGroupMachine;
use crate::node_groups::discovery::NodeGroupDiscoveryObserver;
use crate::node_groups::scaler::NodeGroupScaler;
use crate::node_groups::NodeGroup;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use log::info;
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct NodeGroupInfo {
    node_group: NodeGroup,
    last_discovery: Instant,
    scaler: Option<Addr<NodeGroupScaler>>,
}

pub struct NodeGroupsController {
    node_groups: HashMap<String, Option<NodeGroupMachine>>,
    node_group_max_retain_time: Duration,
    timer: Timer,
    addr: WeakAddr<Self>,
    node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
    cloud_provider: Addr<dyn CloudProvider>,
    dns_provider: Addr<dyn DnsProvider>,
    node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
}

impl NodeGroupsController {
    pub fn new(
        node_discovery_provider: Addr<dyn NodeDiscoveryProvider>,
        cloud_provider: Addr<dyn CloudProvider>,
        dns_provider: Addr<dyn DnsProvider>,
        node_stats_stream_factory: Box<dyn NodeStatsStreamFactory>,
    ) -> Self {
        NodeGroupsController {
            node_groups: HashMap::new(),
            node_group_max_retain_time: Duration::from_secs(10),
            timer: Timer::default(),
            addr: Default::default(),
            node_discovery_provider,
            cloud_provider,
            dns_provider,
            node_stats_stream_factory,
        }
    }
}

impl fmt::Display for NodeGroupsController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeGroupsController")
    }
}

#[async_trait]
impl Actor for NodeGroupsController {
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
impl Tick for NodeGroupsController {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.process_node_groups());
        }

        Produces::ok(())
    }
}

#[async_trait]
impl NodeGroupDiscoveryObserver for NodeGroupsController {
    async fn observe_node_group_discovery(&mut self, node_group: NodeGroup) {
        match self.node_groups.get_mut(&node_group.name) {
            Some(ngmo) => {
                info!("Discovered already known node group {:?}", &node_group.name);
                let ngm = ngmo.take().unwrap();
                *ngmo = Some(ngm.handle(Some(state_machine::Event::Discovered)).await);
            }
            None => {
                info!("Discovered new node group {:?}", &node_group.name);
                self.node_groups.insert(
                    node_group.name.clone(),
                    Some(NodeGroupMachine::new(
                        node_group,
                        self.node_discovery_provider.clone(),
                        self.cloud_provider.clone(),
                        self.dns_provider.clone(),
                        self.node_stats_stream_factory.clone(),
                    )),
                );
            }
        }
    }
}

#[async_trait]
impl NodeDiscoveryObserver for NodeGroupsController {
    async fn observe_node_discovery(&mut self, data: NodeDiscoveryData) {
        if let Some(ngmo) = self.node_groups.get_mut(&data.group) {
            let ngm = ngmo.take().unwrap();
            *ngmo = Some(
                ngm.handle(Some(state_machine::Event::DiscoveredNode {
                    discovery_data: data,
                }))
                .await,
            );
        }
    }
}

#[async_trait]
impl NodeExplorationObserver for NodeGroupsController {
    async fn observe_node_exploration(&mut self, node_info: CloudNodeInfo) {
        if let Some(ngmo) = self.node_groups.get_mut(&node_info.group) {
            let ngm = ngmo.take().unwrap();
            *ngmo = Some(
                ngm.handle(Some(state_machine::Event::ExploredNode { node_info }))
                    .await,
            );
        }
    }
}

impl NodeGroupsController {
    async fn process_node_groups(&mut self) {
        info!("Process node groups");

        for ngmo in self.node_groups.values_mut() {
            let event = match ngmo {
                Some(NodeGroupMachine::Initializing(_)) => Some(state_machine::Event::Initialize {
                    max_retain_time: self.node_group_max_retain_time,
                }),
                _ => None,
            };

            *ngmo = Some(ngmo.take().unwrap().handle(event).await)
        }

        // Remove discarded node groups from the controller
        self.node_groups.retain(|_, ngmo| {
            if let Some(NodeGroupMachine::Discarded(_)) = ngmo {
                false
            } else {
                true
            }
        })
    }
}

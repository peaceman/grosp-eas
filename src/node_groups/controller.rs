mod state_machine;

use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use crate::dns_provider::DnsProvider;
use crate::node::discovery::{NodeDiscoveryData, NodeDiscoveryObserver, NodeDiscoveryProvider};
use crate::node::exploration::NodeExplorationObserver;
use crate::node::stats::NodeStatsStreamFactory;
use crate::node_groups::controller::state_machine::{Event, NodeGroupMachine};
use crate::node_groups::discovery::NodeGroupDiscoveryObserver;
use crate::node_groups::scaler::NodeGroupScaler;
use crate::node_groups::NodeGroup;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{send, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::info;

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
            node_group_max_retain_time: Duration::from_secs(60 * 2),
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

impl fmt::Debug for NodeGroupsController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[async_trait]
impl Actor for NodeGroupsController {
    #[tracing::instrument(skip(addr))]
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
    #[tracing::instrument]
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.process_node_groups());
        }

        Produces::ok(())
    }
}

#[async_trait]
impl NodeGroupDiscoveryObserver for NodeGroupsController {
    #[tracing::instrument]
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
                        self.node_group_max_retain_time.clone(),
                    )),
                );
            }
        }
    }
}

#[async_trait]
impl NodeDiscoveryObserver for NodeGroupsController {
    #[tracing::instrument]
    async fn observe_node_discovery(&mut self, data: NodeDiscoveryData) {
        match self.node_groups.get_mut(&data.group) {
            Some(ngmo) => {
                let ngm = ngmo.take().unwrap();
                *ngmo = Some(
                    ngm.handle(Some(state_machine::Event::DiscoveredNode {
                        discovery_data: data,
                    }))
                    .await,
                );
            }
            None => {
                info!(
                    "Received node discovery for non existing node group; re-initializing {}",
                    data.group
                );

                let group_name = data.group.clone();
                let ngm = self.create_running_node_group_machine(&group_name).await;

                let ngm = ngm
                    .handle(Some(state_machine::Event::DiscoveredNode {
                        discovery_data: data,
                    }))
                    .await;

                self.node_groups.insert(group_name, Some(ngm));
            }
        }
    }
}

#[async_trait]
impl NodeExplorationObserver for NodeGroupsController {
    #[tracing::instrument(skip(self))]
    async fn observe_node_exploration(&mut self, node_info: CloudNodeInfo) {
        match self.node_groups.get_mut(&node_info.group) {
            Some(ngmo) => {
                let ngm = ngmo.take().unwrap();
                *ngmo = Some(
                    ngm.handle(Some(state_machine::Event::ExploredNode { node_info }))
                        .await,
                );
            }
            None => {
                info!(
                    "Received node exploration for non existing node group; re-initializing {}",
                    node_info.group
                );

                let group_name = node_info.group.clone();
                let ngm = self.create_running_node_group_machine(&group_name).await;

                let ngm = ngm
                    .handle(Some(state_machine::Event::ExploredNode { node_info }))
                    .await;

                self.node_groups.insert(group_name, Some(ngm));
            }
        }
    }
}

impl NodeGroupsController {
    #[tracing::instrument]
    async fn process_node_groups(&mut self) {
        info!("Process node groups");

        for ngmo in self.node_groups.values_mut() {
            *ngmo = Some(process_node_group_machine(ngmo.take().unwrap(), None).await)
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

    #[tracing::instrument(fields(group_name = group_name.as_ref()))]
    async fn create_running_node_group_machine(
        &self,
        group_name: impl AsRef<str>,
    ) -> NodeGroupMachine {
        let node_group = NodeGroup {
            name: group_name.as_ref().into(),
        };

        let ngm = NodeGroupMachine::new(
            node_group,
            self.node_discovery_provider.clone(),
            self.cloud_provider.clone(),
            self.dns_provider.clone(),
            self.node_stats_stream_factory.clone(),
            self.node_group_max_retain_time.clone(),
        );

        process_node_group_machine(ngm, Some(state_machine::Event::Initialize)).await
    }
}

#[tracing::instrument]
async fn process_node_group_machine(
    ngm: NodeGroupMachine,
    event: Option<Event>,
) -> NodeGroupMachine {
    ngm.handle(event).await
}

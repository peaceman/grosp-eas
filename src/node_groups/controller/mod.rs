mod state_machine;

use crate::node_groups::controller::state_machine::NodeGroupMachine;
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
}

impl NodeGroupsController {
    pub fn new() -> Self {
        NodeGroupsController {
            node_groups: HashMap::new(),
            node_group_max_retain_time: Duration::from_secs(10),
            timer: Timer::default(),
            addr: Default::default(),
        }
    }
}

impl Default for NodeGroupsController {
    fn default() -> Self {
        Self::new()
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

impl NodeGroupsController {
    pub async fn discovered_node_group(&mut self, node_group: NodeGroup) {
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
                    Some(NodeGroupMachine::new(node_group)),
                );
            }
        }
    }

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

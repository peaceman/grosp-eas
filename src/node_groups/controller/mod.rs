mod state_machine;

use crate::node_groups::controller::state_machine::NodeGroupMachine;
use crate::node_groups::scaler::NodeGroupScalerTrait;
use crate::node_groups::NodeGroup;
use crate::Runtime;
use act_zero::{act_zero, Actor, Addr, Local};
use log::info;
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

type TaskSpawner = Runtime;

#[derive(Debug)]
struct NodeGroupInfo {
    node_group: NodeGroup,
    last_discovery: Instant,
    scaler: Option<Addr<dyn NodeGroupScalerTrait>>,
}

pub struct NodeGroupsController {
    node_groups: HashMap<String, Option<NodeGroupMachine>>,
    task_spawner: TaskSpawner,
    node_group_max_retain_time: Duration,
}

impl NodeGroupsController {
    pub fn new(task_spawner: TaskSpawner) -> Self {
        NodeGroupsController {
            node_groups: HashMap::new(),
            task_spawner,
            node_group_max_retain_time: Duration::from_secs(10),
        }
    }
}

impl fmt::Display for NodeGroupsController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeGroupsController")
    }
}

impl Actor for NodeGroupsController {
    type Error = ();

    fn started(&mut self, addr: Addr<Local<NodeGroupsController>>) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        info!("Started {}", self);

        addr.timer_loop(Duration::from_millis(50));

        Ok(())
    }
}

#[act_zero]
pub trait NodeGroupsControllerTrait {
    fn timer_loop(&self, period: Duration);
    fn discovered_node_group(&self, node_group: NodeGroup);
    fn process_node_groups(&self);
}

#[act_zero]
impl NodeGroupsControllerTrait for NodeGroupsController {
    async fn timer_loop(self: Addr<Local<NodeGroupsController>>, period: Duration) {
        info!("Start timer with a period of {:?}", period);

        let mut interval = tokio::time::interval(period);

        loop {
            interval.tick().await;

            self.process_node_groups();
        }
    }

    async fn discovered_node_group(&mut self, node_group: NodeGroup) {
        match self.node_groups.get_mut(&node_group.name) {
            Some(ngmo) => {
                info!("Discovered already known node group {:?}", &node_group.name);
                let ngm = ngmo.take().unwrap();
                *ngmo = Some(ngm.handle(Some(state_machine::Event::Discovered)));
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
        for ngmo in self.node_groups.values_mut() {
            let event = match ngmo {
                Some(NodeGroupMachine::Initializing(_)) => Some(state_machine::Event::Initialize {
                    spawner: self.task_spawner,
                    max_retain_time: self.node_group_max_retain_time,
                }),
                _ => None,
            };

            *ngmo = Some(ngmo.take().unwrap().handle(event))
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

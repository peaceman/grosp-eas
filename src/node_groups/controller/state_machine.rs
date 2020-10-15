use crate::node_groups::controller::TaskSpawner;
use crate::node_groups::scaler::{NodeGroupScaler, NodeGroupScalerTrait, NodeGroupScalerTraitExt};
use crate::node_groups::NodeGroup;
use act_zero::{channel, spawn, Addr};
use log::info;
use std::time::{Duration, Instant};

pub enum Event {
    Initialize {
        spawner: TaskSpawner,
        max_retain_time: Duration,
    },
    Discovered,
    Discard,
}

trait Handler {
    fn handle(self, event: Option<Event>) -> NodeGroupMachine;
}

#[derive(Debug)]
pub struct Data<S> {
    node_group: NodeGroup,
    state: S,
}

#[derive(Debug)]
pub struct Initializing;

impl Handler for Data<Initializing> {
    fn handle(self, event: Option<Event>) -> NodeGroupMachine {
        match event {
            Some(Event::Initialize {
                spawner,
                max_retain_time,
            }) => {
                let scaler =
                    spawn(&spawner, NodeGroupScaler::new(self.node_group.clone())).unwrap();

                NodeGroupMachine::Running(Data {
                    node_group: self.node_group,
                    state: Running {
                        scaler: scaler.upcast::<dyn NodeGroupScalerTrait>(),
                        last_discovery: Instant::now(),
                        max_retain_time,
                    },
                })
            }
            _ => NodeGroupMachine::Initializing(self),
        }
    }
}

#[derive(Debug)]
pub struct Running {
    scaler: Addr<dyn NodeGroupScalerTrait>,
    last_discovery: Instant,
    max_retain_time: Duration,
}

impl Handler for Data<Running> {
    fn handle(self, event: Option<Event>) -> NodeGroupMachine {
        match event {
            Some(Event::Discovered) => NodeGroupMachine::Running(Data {
                node_group: self.node_group,
                state: Running {
                    last_discovery: Instant::now(),
                    ..self.state
                },
            }),
            Some(Event::Discard) => NodeGroupMachine::Discarding(Data {
                node_group: self.node_group,
                state: Discarding::new(self.state.scaler),
            }),
            None => self.check_last_discovery(),
            _ => NodeGroupMachine::Running(self),
        }
    }
}

impl Data<Running> {
    fn check_last_discovery(self) -> NodeGroupMachine {
        let should_discard =
            Instant::now().duration_since(self.state.last_discovery) > self.state.max_retain_time;

        if should_discard {
            self.handle(Some(Event::Discard))
        } else {
            NodeGroupMachine::Running(self)
        }
    }
}

#[derive(Debug)]
pub struct Discarding {
    scaler: Addr<dyn NodeGroupScalerTrait>,
    triggered_termination: bool,
}

impl Discarding {
    fn new(scaler: Addr<dyn NodeGroupScalerTrait>) -> Self {
        Discarding {
            scaler,
            triggered_termination: false,
        }
    }
}

impl Handler for Data<Discarding> {
    fn handle(mut self, _event: Option<Event>) -> NodeGroupMachine {
        if !self.state.triggered_termination {
            info!(
                "Trigger NodeGroupScaler termination {}",
                self.node_group.name
            );
            self.state.triggered_termination = true;
            self.state.scaler.terminate();
        }

        let (sender, mut receiver) = channel();
        self.state.scaler.is_alive(sender);

        // When the channel returns an error the actor is dropped and
        // we can fully discard this node group
        if receiver.try_recv().is_err() {
            info!("Terinated NodeGroupScaler {}", self.node_group.name);

            NodeGroupMachine::Discarded(Data {
                node_group: self.node_group,
                state: Discarded,
            })
        } else {
            NodeGroupMachine::Discarding(self)
        }
    }
}

#[derive(Debug)]
pub struct Discarded;

#[derive(Debug)]
pub enum NodeGroupMachine {
    Initializing(Data<Initializing>),
    Running(Data<Running>),
    Discarding(Data<Discarding>),
    Discarded(Data<Discarded>),
}

impl NodeGroupMachine {
    pub fn new(node_group: NodeGroup) -> Self {
        Self::Initializing(Data {
            node_group,
            state: Initializing,
        })
    }

    pub fn handle(self, event: Option<Event>) -> Self {
        match self {
            Self::Initializing(m) => m.handle(event),
            Self::Running(m) => m.handle(event),
            Self::Discarding(m) => m.handle(event),
            Self::Discarded(_) => self,
        }
    }
}

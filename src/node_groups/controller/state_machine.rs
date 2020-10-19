use crate::node_groups::scaler::NodeGroupScaler;
use crate::node_groups::NodeGroup;
use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{send, Addr, AddrLike};
use async_trait::async_trait;
use log::info;
use std::time::{Duration, Instant};

pub enum Event {
    Initialize { max_retain_time: Duration },
    Discovered,
    Discard,
}

#[async_trait]
trait Handler {
    async fn handle(self, event: Option<Event>) -> NodeGroupMachine;
}

#[derive(Debug)]
pub struct Data<S> {
    node_group: NodeGroup,
    state: S,
}

#[derive(Debug)]
pub struct Initializing;

#[async_trait]
impl Handler for Data<Initializing> {
    async fn handle(self, event: Option<Event>) -> NodeGroupMachine {
        match event {
            Some(Event::Initialize { max_retain_time }) => {
                let scaler = spawn_actor(NodeGroupScaler::new(self.node_group.clone()));

                NodeGroupMachine::Running(Data {
                    node_group: self.node_group,
                    state: Running {
                        scaler,
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
    scaler: Addr<NodeGroupScaler>,
    last_discovery: Instant,
    max_retain_time: Duration,
}

#[async_trait]
impl Handler for Data<Running> {
    async fn handle(self, event: Option<Event>) -> NodeGroupMachine {
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
            None => self.check_last_discovery().await,
            _ => NodeGroupMachine::Running(self),
        }
    }
}

impl Data<Running> {
    async fn check_last_discovery(self) -> NodeGroupMachine {
        let should_discard =
            Instant::now().duration_since(self.state.last_discovery) > self.state.max_retain_time;

        if should_discard {
            self.handle(Some(Event::Discard)).await
        } else {
            NodeGroupMachine::Running(self)
        }
    }
}

#[derive(Debug)]
pub struct Discarding {
    scaler: Addr<NodeGroupScaler>,
    triggered_termination: bool,
}

impl Discarding {
    fn new(scaler: Addr<NodeGroupScaler>) -> Self {
        Discarding {
            scaler,
            triggered_termination: false,
        }
    }
}

#[async_trait]
impl Handler for Data<Discarding> {
    async fn handle(mut self, _event: Option<Event>) -> NodeGroupMachine {
        if !self.state.triggered_termination {
            info!(
                "Trigger NodeGroupScaler termination {}",
                self.node_group.name
            );
            self.state.triggered_termination = true;
            send!(self.state.scaler.terminate());
        }

        let scaler_is_terminated = tokio::select! {
            _ = self.state.scaler.termination() => {
                true
            }
            _ = async { true } => {
                false
            }
        };

        if scaler_is_terminated {
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

    pub async fn handle(self, event: Option<Event>) -> Self {
        match self {
            Self::Initializing(m) => m.handle(event).await,
            Self::Running(m) => m.handle(event).await,
            Self::Discarding(m) => m.handle(event).await,
            Self::Discarded(_) => self,
        }
    }
}

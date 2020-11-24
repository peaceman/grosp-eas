use crate::cloud_provider::{CloudNodeInfo, CloudProvider};
use act_zero::{call, send, Actor, ActorError, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use std::time::Duration;
use tracing::{error, info};

use crate::actor;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;

#[async_trait]
pub trait NodeExplorationObserver: Actor {
    async fn observe_node_exploration(&mut self, node_info: CloudNodeInfo);
}

pub struct NodeExploration {
    provider: Addr<dyn CloudProvider>,
    observer: Addr<dyn NodeExplorationObserver>,
    exploration_interval: Duration,
    addr: WeakAddr<Self>,
    timer: Timer,
}

impl NodeExploration {
    pub fn new(
        provider: Addr<dyn CloudProvider>,
        observer: Addr<dyn NodeExplorationObserver>,
        exploration_interval: Duration,
    ) -> Self {
        Self {
            provider,
            observer,
            exploration_interval,
            addr: Default::default(),
            timer: Default::default(),
        }
    }

    #[tracing::instrument(name = "NodeExploration::explore", skip(self))]
    async fn explore(&mut self) {
        let explorations = call!(self.provider.get_nodes()).await;

        match explorations {
            Ok(explorations) => {
                for exploration in explorations {
                    send!(self.observer.observe_node_exploration(exploration));
                }
            }
            Err(e) => error!(
                error = format!("{:?}", e).as_str(),
                "Failed to explore nodes"
            ),
        }
    }
}

#[async_trait]
impl Actor for NodeExploration {
    #[tracing::instrument(name = "NodeExploration::started", skip(self, addr))]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

        self.addr = addr.downgrade();
        self.timer
            .set_interval_weak(self.addr.clone(), self.exploration_interval);

        Produces::ok(())
    }

    async fn error(&mut self, error: ActorError) -> bool {
        actor::handle_error(error)
    }
}

#[async_trait]
impl Tick for NodeExploration {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.explore());
        }

        Produces::ok(())
    }
}

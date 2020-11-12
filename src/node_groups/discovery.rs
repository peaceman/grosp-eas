pub mod provider;

use crate::node_groups::discovery::provider::NodeGroupDiscoveryProvider;
use crate::node_groups::NodeGroup;
use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use act_zero::{call, send};
use act_zero::{Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use std::time::Duration;
use tracing::{error, info};

pub use provider::file::FileNodeGroupDiscovery;

#[async_trait]
pub trait NodeGroupDiscoveryObserver: Actor {
    async fn observe_node_group_discovery(&mut self, node_group: NodeGroup);
}

pub struct NodeGroupDiscovery {
    provider: Addr<dyn NodeGroupDiscoveryProvider>,
    observer: Addr<dyn NodeGroupDiscoveryObserver>,
    interval: Duration,
    timer: Timer,
    addr: WeakAddr<Self>,
}

impl NodeGroupDiscovery {
    pub fn new(
        provider: Addr<dyn NodeGroupDiscoveryProvider>,
        observer: Addr<dyn NodeGroupDiscoveryObserver>,
        interval: Duration,
    ) -> Self {
        Self {
            provider,
            observer,
            interval,
            timer: Default::default(),
            addr: Default::default(),
        }
    }

    async fn discover(&mut self) {
        let discoveries = call!(self.provider.discover_node_groups()).await;

        match discoveries {
            Ok(discoveries) => {
                for discovery in discoveries {
                    send!(self.observer.observe_node_group_discovery(discovery));
                }
            }
            Err(e) => error!(
                error = format!("{:?}", e).as_str(),
                "Failed to discovery node groups"
            ),
        }
    }
}

#[async_trait]
impl Actor for NodeGroupDiscovery {
    #[tracing::instrument(
        name = "NodeGroupDiscovery::started"
        skip(self, addr),
    )]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

        self.addr = addr.downgrade();

        self.timer
            .set_interval_weak(self.addr.clone(), self.interval);

        Produces::ok(())
    }
}

#[async_trait]
impl Tick for NodeGroupDiscovery {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.discover());
        }

        Produces::ok(())
    }
}

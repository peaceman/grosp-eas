pub mod provider;

use crate::node::NodeDrainingCause;
use act_zero::{call, send, Actor, ActorResult, Addr, Produces, WeakAddr};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use act_zero::runtimes::tokio::Timer;
use act_zero::timer::Tick;
use std::convert::TryFrom;
use std::time::Duration;
use tokio::time::Interval;
use tracing::{error, info};

use crate::consul::health::ServiceEntry;
pub use provider::NodeDiscoveryProvider;

#[async_trait]
pub trait NodeDiscoveryObserver: Actor {
    async fn observe_node_discovery(&mut self, data: NodeDiscoveryData);
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NodeDiscoveryData {
    pub hostname: String,
    pub group: String,
    pub state: NodeDiscoveryState,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum NodeDiscoveryState {
    Ready,
    Active,
    Draining(NodeDrainingCause),
}

pub struct NodeDiscovery {
    provider: Addr<dyn NodeDiscoveryProvider>,
    observer: Addr<dyn NodeDiscoveryObserver>,
    discovery_interval: Duration,
    addr: WeakAddr<Self>,
    timer: Timer,
}

impl NodeDiscovery {
    pub fn new(
        provider: Addr<dyn NodeDiscoveryProvider>,
        observer: Addr<dyn NodeDiscoveryObserver>,
        discovery_interval: Duration,
    ) -> Self {
        Self {
            provider,
            observer,
            discovery_interval,
            addr: Default::default(),
            timer: Default::default(),
        }
    }

    #[tracing::instrument(
        name = "NodeDiscovery::discover"
        skip(self)
    )]
    async fn discover(&mut self) {
        let discoveries = call!(self.provider.discover_nodes()).await;

        match discoveries {
            Ok(discoveries) => {
                for discovery in discoveries {
                    send!(self.observer.observe_node_discovery(discovery));
                }
            }
            Err(e) => error!(
                error = format!("{:?}", e).as_str(),
                "Failed to discover nodes"
            ),
        }
    }
}

#[async_trait]
impl Actor for NodeDiscovery {
    #[tracing::instrument(
        name = "NodeDiscovery::started"
        skip(self, addr),
    )]
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started");

        self.addr = addr.downgrade();

        self.timer
            .set_interval_weak(self.addr.clone(), self.discovery_interval);

        Produces::ok(())
    }
}

#[async_trait]
impl Tick for NodeDiscovery {
    async fn tick(&mut self) -> ActorResult<()> {
        if self.timer.tick() {
            send!(self.addr.discover());
        }

        Produces::ok(())
    }
}

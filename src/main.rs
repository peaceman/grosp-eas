use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{call, upcast, Actor, ActorResult, Produces};
use async_trait::async_trait;
use chrono::Utc;
use edge_auto_scaler::cloud_provider::CloudNodeInfo;
use edge_auto_scaler::node::discovery::{
    FileNodeDiscovery, NodeDiscoveryData, NodeDiscoveryProvider, NodeDiscoveryState,
};
use edge_auto_scaler::node::exploration::FileNodeExploration;
use edge_auto_scaler::node::stats::NodeStatsStreamFactory;
use edge_auto_scaler::node::{NodeController, NodeDrainingCause, NodeStats};
use edge_auto_scaler::node_groups::discovery::FileNodeGroupDiscovery;
use edge_auto_scaler::node_groups::NodeGroupsController;
use env_logger::Env;
use futures::task::Context;
use log::info;
use std::net::IpAddr;
use std::time::Duration;
use tokio::macros::support::{Pin, Poll};
use tokio::stream::Stream;

fn init_logging() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let stream_factory = Box::new(StreamFactory);
    let node_discovery_provider = spawn_actor(MockNodeDiscovery);

    let node_groups_controller = spawn_actor(NodeGroupsController::new(
        upcast!(node_discovery_provider),
        Default::default(),
        Default::default(),
        stream_factory.clone(),
    ));

    let _node_group_discovery = spawn_actor(FileNodeGroupDiscovery::new(
        "test_files/node_groups",
        upcast!(node_groups_controller.clone()),
    ));
    let _node_exploration = spawn_actor(FileNodeExploration::new(
        "test_files/node_exploration",
        upcast!(node_groups_controller.clone()),
    ));
    let _node_discovery = spawn_actor(FileNodeDiscovery::new(
        "test_files/node_discovery",
        upcast!(node_groups_controller.clone()),
    ));

    // let node_controller = spawn_actor(NodeController::new(
    //     "demo".into(),
    //     Default::default(),
    //     upcast!(node_discovery_provider),
    //     Default::default(),
    //     Default::default(),
    //     stream_factory.clone(),
    // ));
    //
    // call!(node_controller.explored_node(CloudNodeInfo {
    //     identifier: "fock".into(),
    //     hostname: "demo".into(),
    //     group: "lel".into(),
    //     ip_addresses: vec!["127.0.0.1".parse().unwrap()],
    //     created_at: Utc::now(),
    // }))
    // .await?;
    // call!(node_controller.discovered_node(NodeDiscoveryData {
    //     group: "lel".into(),
    //     hostname: "demo".into(),
    //     state: NodeDiscoveryState::Ready,
    // }))
    // .await?;
    //
    // call!(node_controller.activate_node()).await?;
    //
    // tokio::time::delay_for(Duration::from_secs(15)).await;
    // call!(node_controller.deprovision_node(NodeDrainingCause::Scaling)).await;

    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = interval.tick() => ()
        }
    }
}

#[derive(Clone, Debug)]
struct StreamFactory;

impl NodeStatsStreamFactory for StreamFactory {
    fn create_stream(&self, hostname: String) -> Box<dyn Stream<Item = NodeStats> + Unpin + Send> {
        info!("Creating NodeStatsStream for {}", hostname);
        Box::new(FixedNodeStatsStream {
            interval: tokio::time::interval(Duration::from_millis(100)),
        })
    }
}

struct FixedNodeStatsStream {
    interval: tokio::time::Interval,
}

impl Stream for FixedNodeStatsStream {
    type Item = NodeStats;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.interval).poll_next(cx) {
            Poll::Ready(_) => Poll::Ready(Some(NodeStats {
                tx_bps: 23,
                rx_bps: 23,
            })),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for FixedNodeStatsStream {
    fn drop(&mut self) {
        info!("Drop FixedNodeStatsStream");
    }
}

struct MockNodeDiscovery;

impl Actor for MockNodeDiscovery {}

#[async_trait]
impl NodeDiscoveryProvider for MockNodeDiscovery {
    async fn update_state(
        &mut self,
        hostname: String,
        state: NodeDiscoveryState,
    ) -> ActorResult<()> {
        info!("Updating state of node {} {:?}", hostname, state);

        Produces::ok(())
    }
}

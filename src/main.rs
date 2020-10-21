use act_zero::runtimes::tokio::spawn_actor;
use edge_auto_scaler::node::{NodeController, NodeStats};
use edge_auto_scaler::node_groups::discovery::FileBasedNodeGroupExplorer;
use edge_auto_scaler::node_groups::NodeGroupsController;
use env_logger::Env;
use futures::task::Context;
use std::time::Duration;
use tokio::macros::support::{Pin, Poll};
use tokio::stream::Stream;

fn init_logging() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let node_groups_controller = spawn_actor(NodeGroupsController::new());
    let _explorer = spawn_actor(FileBasedNodeGroupExplorer::new(
        "node_groups",
        node_groups_controller,
    ));

    let _node_controller = spawn_actor(NodeController::new(
        "demo".into(),
        Default::default(),
        Default::default(),
        Default::default(),
        |_hostname| FixedNodeStatsStream {
            interval: tokio::time::interval(Duration::from_millis(100)),
        },
    ));

    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = interval.tick() => ()
        }
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

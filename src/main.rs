use act_zero::runtimes::tokio::spawn_actor;
use edge_auto_scaler::node_groups::discovery::FileBasedNodeGroupExplorer;
use edge_auto_scaler::node_groups::NodeGroupsController;
use env_logger::Env;
use std::time::Duration;

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

    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = interval.tick() => ()
        }
    }
}

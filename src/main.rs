use act_zero::runtimes::tokio::spawn_actor;
use act_zero::upcast;
use edge_auto_scaler::config::load_config;
use edge_auto_scaler::node::discovery::NodeDiscovery;
use edge_auto_scaler::node::exploration::NodeExploration;
use edge_auto_scaler::node::stats::{build_stream_factory_from_config, NodeStatsStreamFactory};
use edge_auto_scaler::node::NodeStats;
use edge_auto_scaler::node_groups::discovery::NodeGroupDiscovery;
use edge_auto_scaler::node_groups::NodeGroupsController;
use edge_auto_scaler::{cloud_provider, dns_provider, node, node_groups};
use futures::task::Context;
use opentelemetry::api::Provider;
use opentelemetry::sdk;
use std::sync::Arc;
use std::time::Duration;
use tokio::macros::support::{Pin, Poll};
use tokio::stream::Stream;
use tracing::info;
use tracing::subscriber::set_global_default;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    // env_logger::from_env(Env::default().default_filter_or("info")).init();
    let exporter = opentelemetry_jaeger::Exporter::builder()
        .with_agent_endpoint("127.0.0.1:6831".parse()?)
        .with_process(opentelemetry_jaeger::Process {
            service_name: "edge_auto_scaler".into(),
            tags: vec![],
        })
        .init()
        .expect("Error initializing Jaeger exporter");

    let provider = sdk::Provider::builder()
        .with_simple_exporter(exporter)
        .with_config(sdk::Config {
            default_sampler: Box::new(sdk::Sampler::AlwaysOn),
            ..Default::default()
        })
        .build();

    let telemetry = tracing_opentelemetry::layer().with_tracer(provider.get_tracer(""));

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));

    let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(telemetry)
        .with(fmt_layer);

    set_global_default(subscriber).expect("Failed to set subscriber");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging()?;

    let config = load_config()?;
    let node_group_scaler_config = Arc::new(config.node_group_scaler.clone());

    let stream_factory = build_stream_factory_from_config(Arc::clone(&config))?;
    let cloud_provider = cloud_provider::build_from_config(Arc::clone(&config))?;
    let dns_provider = dns_provider::build_from_config(Arc::clone(&config))?;
    let node_discovery_provider =
        node::discovery::provider::build_from_config(Arc::clone(&config))?;

    let node_group_discovery_providers =
        node_groups::discovery::provider::build_from_config(Arc::clone(&config))?;

    let node_groups_controller = spawn_actor(NodeGroupsController::new(
        node_discovery_provider.clone(),
        cloud_provider.clone(),
        dns_provider.clone(),
        stream_factory.clone(),
        Arc::new(node_group_scaler_config.node_hostname_suffix.clone()),
        Arc::clone(&node_group_scaler_config),
    ));

    let _node_discovery = spawn_actor(NodeDiscovery::new(
        node_discovery_provider.clone(),
        upcast!(node_groups_controller.clone()),
        config.node_discovery.interval,
    ));

    let _node_exploration = spawn_actor(NodeExploration::new(
        cloud_provider.clone(),
        upcast!(node_groups_controller.clone()),
        config.node_exploration.interval,
    ));

    let _node_group_discovery = spawn_actor(NodeGroupDiscovery::new(
        node_group_discovery_providers.clone(),
        upcast!(node_groups_controller.clone()),
        config.node_group_discovery.interval,
    ));

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
    fn create_stream(&self, hostname: String) -> Pin<Box<dyn Stream<Item = NodeStats> + Send>> {
        info!("Creating NodeStatsStream for {}", hostname);
        Box::pin(FixedNodeStatsStream {
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
                tx_bps: 100,
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

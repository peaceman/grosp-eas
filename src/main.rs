use act_zero::runtimes::tokio::spawn_actor;
use act_zero::{call, upcast, Actor, ActorResult, Addr, Produces};
use async_trait::async_trait;
use chrono::Utc;
use edge_auto_scaler::cloud_provider::{CloudNodeInfo, CloudProvider, FileCloudProvider};
use edge_auto_scaler::dns_provider::DnsProvider;
use edge_auto_scaler::node::discovery::{
    FileNodeDiscovery, NodeDiscoveryData, NodeDiscoveryProvider, NodeDiscoveryState,
};
use edge_auto_scaler::node::exploration::FileNodeExploration;
use edge_auto_scaler::node::stats::{
    FileNodeStatsStream, FileNodeStatsStreamFactory, NSSStreamFactory, NodeStatsStreamFactory,
};
use edge_auto_scaler::node::{NodeController, NodeDrainingCause, NodeStats};
use edge_auto_scaler::node_groups::discovery::FileNodeGroupDiscovery;
use edge_auto_scaler::node_groups::NodeGroupsController;
use env_logger::Env;
use futures::task::Context;
use opentelemetry::api::Provider;
use opentelemetry::sdk;
use rand::Rng;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::macros::support::{Pin, Poll};
use tokio::stream::{Stream, StreamExt};
use tracing::info;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::fmt::Subscriber;
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

    let nss_stream_factory = NSSStreamFactory::new(
        tokio::fs::read("/Users/peaceman/Development/ops/grosp-hcloud/data/pki/main/ca/ca.pem")
            .await?,
        tokio::fs::read(
            "/Users/peaceman/Development/ops/grosp-hcloud/data/pki/main/certs/nss-alpha.pem",
        )
        .await?,
        tokio::fs::read(
            "/Users/peaceman/Development/ops/grosp-hcloud/data/pki/main/certs/nss-alpha-key.p8",
        )
        .await?,
        "nss-edge-node".into(),
    );

    let mut stream = nss_stream_factory.create_stream("localhost".into());
    while let Some(stats) = stream.next().await {
        info!("STATS: {:?}", stats);
    }

    let stream_factory = Box::new(StreamFactory);
    let node_discovery_provider = spawn_actor(MockNodeDiscovery);
    let cloud_provider = spawn_actor(FileCloudProvider::new(
        "test_files/node_exploration",
        "test_files/node_discovery",
    ));
    let dns_provider = spawn_actor(MockDnsProvider);

    let node_groups_controller = spawn_actor(NodeGroupsController::new(
        upcast!(node_discovery_provider),
        upcast!(cloud_provider),
        upcast!(dns_provider),
        stream_factory.clone(),
        // Arc::new(vec![
        //     "beta.gt.n2305.link",
        //     "gamma.gt.n2305.link",
        //     "delta.gt.n2305.link",
        //     "epsilon.gt.n2305.link",
        //     "psi.gt.n2305.link",
        // ]),
        // Arc::new(('a'..='z').collect::<Vec<char>>()),
        Arc::new("gt.n2305.link".to_owned()),
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

struct MockCloudProvider;

#[async_trait]
impl Actor for MockCloudProvider {
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started MockCloudProvider");

        Produces::ok(())
    }
}

#[async_trait]
impl CloudProvider for MockCloudProvider {
    async fn get_node_info(&mut self, hostname: String) -> ActorResult<Option<CloudNodeInfo>> {
        Produces::ok(None)
    }

    async fn create_node(
        &mut self,
        hostname: String,
        target_state: NodeDiscoveryState,
    ) -> ActorResult<CloudNodeInfo> {
        unimplemented!()
    }

    async fn delete_node(&mut self, node_info: CloudNodeInfo) -> ActorResult<()> {
        Produces::ok(())
    }
}

struct MockDnsProvider;

#[async_trait]
impl Actor for MockDnsProvider {
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        info!("Started MockDnsProvider");

        Produces::ok(())
    }
}

#[async_trait]
impl DnsProvider for MockDnsProvider {
    async fn create_records(
        &mut self,
        hostname: String,
        ip_addresses: Vec<IpAddr>,
    ) -> ActorResult<()> {
        Produces::ok(())
    }

    async fn delete_records(&mut self, hostname: String) -> ActorResult<()> {
        Produces::ok(())
    }
}

use crate::node::stats::NodeStatsStreamFactory;
use crate::node::NodeStats;
use async_stream::stream;
use http::Uri;
use std::pin::Pin;
use std::time::Duration;
use tokio::stream::Stream;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};
use tracing::error;
use tracing_futures::Instrument;

pub mod grpc {
    tonic::include_proto!("nodestats");
}

use grpc::{node_stats_service_client::NodeStatsServiceClient, LiveNodeStatsRequest};

#[derive(Clone, Debug)]
pub struct NSSStreamFactory {
    client_tls_config: ClientTlsConfig,
    service_port: u16,
}

impl NSSStreamFactory {
    pub fn new(
        ca_cert: Vec<u8>,
        client_cert: Vec<u8>,
        client_key: Vec<u8>,
        sni_domain_name: String,
        service_port: u16,
    ) -> Self {
        Self {
            client_tls_config: ClientTlsConfig::new()
                .domain_name(sni_domain_name)
                .ca_certificate(Certificate::from_pem(ca_cert))
                .identity(Identity::from_pem(client_cert, client_key)),
            service_port,
        }
    }
}

impl NodeStatsStreamFactory for NSSStreamFactory {
    fn create_stream(&self, hostname: String) -> Pin<Box<dyn Stream<Item = NodeStats> + Send>> {
        let client_tls_config = self.client_tls_config.clone();
        let uri = Uri::builder()
            .scheme("https")
            .authority(format!("{}:{}", hostname, self.service_port).as_str())
            .path_and_query("")
            .build()
            .unwrap();

        let tracing_span = tracing::info_span!("NodeStatsStream", uri = uri.to_string().as_str());

        let mut first_try = true;
        let stream = stream! {
            loop {
                if first_try {
                    first_try = false;
                } else {
                    tokio::time::delay_for(Duration::from_secs(10)).await;
                }

                let channel = Channel::builder(uri.clone())
                    .tls_config(client_tls_config.clone());

                if let Err(e) = channel {
                    error!(
                        uri = uri.to_string().as_str(),
                        error = format!("{:?}", e).as_str(),
                        "Failed to create channel"
                    );

                    continue;
                }

                let channel = channel.unwrap().connect().await;

                if let Err(e) = channel {
                    error!(

                        error = format!("{:?}", e).as_str(),
                        "Failed to establish connection"
                    );

                    continue;
                }

                let channel = channel.unwrap();
                let mut client = NodeStatsServiceClient::new(channel);

                let request = tonic::Request::new(LiveNodeStatsRequest {});
                let response = client.get_live_stats(request).await;

                if let Err(e) = response {
                    error!(
                        uri = uri.to_string().as_str(),
                        error = format!("{:?}", e).as_str(),
                        "Request failed",
                    );

                    continue;
                }

                let response = response.unwrap();
                let mut stream = response.into_inner();

                while let Ok(Some(grpc::NodeStats {
                    used_bandwidth: Some(bandwidth)
                })) = stream.message().await {
                    yield NodeStats {
                        tx_bps: bandwidth.tx_bps,
                        rx_bps: bandwidth.rx_bps,
                    }
                }
            }
        };

        Box::pin(stream.instrument(tracing_span))
    }
}

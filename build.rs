fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile(&["proto/nodestats/node_stats.proto"], &["proto/nodestats"])?;

    Ok(())
}

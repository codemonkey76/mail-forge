use log::info;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::config::{load_certs, structs::Config};

pub async fn start(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(&config.server.smtp_bind_address).await?;
    info!(
        "Starting SMTP server on {}",
        config.server.smtp_bind_address
    );
    let tls_config = load_certs(
        config.server.cert_path.clone().into(),
        config.server.key_path.clone().into(),
    )?;

    let tls_config = Arc::new(tls_config); // Wrap in Arc for thread-safe sharing
    let config = Arc::new(config);
    loop {
        let (socket, addr) = listener.accept().await?;
        info!("Connection from {}", addr);

        let config = Arc::clone(&config);
        let tls_config = tls_config.clone();
        tokio::spawn(async move {
            super::handler::handle_client(socket, tls_config, addr, config).await;
        });
    }
}

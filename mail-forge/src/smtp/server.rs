use crate::config::Config;
use log::info;
use std::sync::Arc;
use tokio::net::TcpListener;

pub async fn start(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(&config.smtp_bind_address).await?;
    info!("Starting SMTP server on {}", config.smtp_bind_address);

    let mapping = Arc::new(config.webhooks);
    loop {
        let (socket, addr) = listener.accept().await?;
        info!("Connection from {}", addr);

        let mapping = Arc::clone(&mapping);
        tokio::spawn(async move {
            super::handler::handle_client(socket, addr, mapping).await;
        });
    }
}

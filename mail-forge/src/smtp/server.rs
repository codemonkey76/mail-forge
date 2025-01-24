use crate::config::Config;
use log::info;
use tokio::net::TcpListener;

use super::handler::WebhookMapping;

pub async fn start(
    config: Config,
    webhook_mapping: WebhookMapping,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(&config.smtp_bind_address).await?;
    info!("Starting SMTP server on {}", config.smtp_bind_address);

    loop {
        let (socket, addr) = listener.accept().await?;
        info!("Connection from {}", addr);

        tokio::spawn(async move {
            super::handler::handle_client(socket, addr, webhook_mapping.clone()).await;
        });
    }
}

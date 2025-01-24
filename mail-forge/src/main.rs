mod config;
mod smtp;
mod webhook;

use log::info;
use smtp::handler::load_webhook_mapping;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    env_logger::init();
    info!("Staring Mail Forge...");

    // Load configuration
    let config = config::loader::load_config()?;
    let mapping = load_webhook_mapping();

    // Start the SMTP server
    smtp::server::start(config, mapping).await?;

    Ok(())
}

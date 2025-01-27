use std::env;
use mail_forge::{config, smtp};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    env_logger::init();

    let config = config::Config::load("config.toml")?;

    smtp::server::start(config).await?;

    Ok(())
}

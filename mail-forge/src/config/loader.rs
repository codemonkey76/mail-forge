use crate::config::Config;
use dotenv::dotenv;
use std::env;

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    dotenv().ok();

    let smtp_bind_address =
        env::var("SMTP_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:2525".to_string());
    let webhook_url = env::var("WEBHOOK_URL").expect("WEBHOOK_URL must be set");

    Ok(Config {
        smtp_bind_address,
        webhook_url,
    })
}

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub webhooks: HashMap<String, WebhookConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub smtp_bind_address: String,
    pub hostname: String,
    pub max_size: usize,
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    pub api_key: String,
}

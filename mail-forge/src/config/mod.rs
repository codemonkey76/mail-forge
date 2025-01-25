use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub smtp_bind_address: String,
    pub webhooks: HashMap<String, String>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(file_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let config_contents = fs::read_to_string(file_path)?;
        let config: Config = toml::from_str(&config_contents)?;
        Ok(config)
    }
}

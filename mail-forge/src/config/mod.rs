pub mod loader;

#[derive(Debug)]
pub struct Config {
    pub smtp_bind_address: String,
    pub webhook_url: String,
}

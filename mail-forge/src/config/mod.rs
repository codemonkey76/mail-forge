pub mod structs;

use rustls::ServerConfig;
use std::fs;
use std::path::{Path, PathBuf};
use structs::Config;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

impl Config {
    pub fn load<P: AsRef<Path>>(file_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let config_contents = fs::read_to_string(file_path)?;
        let config: Config = toml::from_str(&config_contents)?;
        Ok(config)
    }
}

pub fn load_certs(
    cert_path: PathBuf,
    key_path: PathBuf,
) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let certs = CertificateDer::pem_file_iter(cert_path)?.collect::<Result<Vec<_>, _>>()?;
    let key = PrivateKeyDer::from_pem_file(key_path)?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(config)
}

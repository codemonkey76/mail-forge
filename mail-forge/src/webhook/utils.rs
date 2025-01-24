use base64::engine::general_purpose;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn generate_signature(api_key: &str, timestamp: &str, token: &str) -> String {
    let payload = format!("{}{}", timestamp, token);

    let mut mac =
        HmacSha256::new_from_slice(api_key.as_bytes()).expect("HMAC can take a key of any size");

    mac.update(payload.as_bytes());
    let result = mac.finalize();
    let signature_bytes = result.into_bytes();

    general_purpose::STANDARD.encode(signature_bytes)
}

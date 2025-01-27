use chrono::Utc;
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde_json::json;

use crate::config::structs::WebhookConfig;
use crate::webhook::utils::generate_signature;

pub async fn forward_to_webhook(
    webhook: &WebhookConfig,
    raw_email: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let api_key = webhook.api_key.clone();

    let timestamp = Utc::now().timestamp().to_string();

    let token: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let signature = generate_signature(&api_key, &timestamp, &token);

    let payload = json!({
        "email": raw_email,
        "timestamp": timestamp,
        "token": token,
        "signature": signature,
    });

    info!("Payload being sent to webhook: {}", payload);

    let response = client.post(webhook.url.clone()).json(&payload).send().await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_else(|_| "No body".to_string());

            if status.is_success() {
                info!("Successfully forwarded to webhook: {}", webhook.url);
                Ok(())
            } else {
                error!(
                    "Webhook responded with error. Status: {}, Body: {}",
                    status, body
                );
                Err(format!("Webhook returned status: {}", status).into())
            }
        }
        Err(err) => {
            error!("Failed to send webhook request to {}: {}", webhook.url, err);
            Err(err.into())
        }
    }
}

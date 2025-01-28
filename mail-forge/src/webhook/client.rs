use std::env::temp_dir;
use std::fs::File;
use std::path;
use chrono::Utc;
use log::{error, info};
use mailparse::MailHeaderMap;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::{multipart, Client};
use serde_json::json;
use crate::config;
use crate::webhook::utils::generate_signature;

pub async fn forward_to_webhook(
    webhook: &config::WebhookConfig,
    raw_email: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let api_key = webhook.api_key.clone();

    let (timestamp, token, signature) = generate_auth(&webhook.api_key);

    // Parse email and extract attachments
    let attachments = extract_attachments(raw_email)?;

    // Save all attachments to temporary files
    let temp_files = save_attachments_to_temp_files(&attachments)?;

    // Create multipart form
    let form = create_multipart_form(raw_email, &timestamp, &token, &signature, &temp_files)?;

    // Send to webhook
    send_to_webhook(&client, &webhook.url, form).await?;

    Ok(())
}

async fn send_to_webhook(client: &Client, webhook_url: &str,
                         form: multipart::Form) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.post(webhook_url).multipart(form).send().await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_else(|_| "No body".to_string());

            if status.is_success() {
                info!("Successfully forward to webhook: {}", webhook_url);
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
            error!("Failed to send webhook request to {}: {}", webhook_url, err);
            Err(err.into())
        }
    }
}
fn generate_auth(api_key: &str) -> (String, String, String) {
    let timestamp = Utc::now().timestamp().to_string();

    let token: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let signature = generate_signature(&api_key, &timestamp, &token);

    (timestamp, token, signature)
}

fn extract_attachments(raw_email: &str) -> Result<Vec<(String, Vec<u8>)>, Box<dyn std::error::Error>> {
    let parsed_mail = mailparse::parse_mail(raw_email.as_bytes())?;
    let mut attachments = Vec::new();

    for part in parsed_mail.subparts {
        if let Some(content_disposition) = part.get_headers().get_first_value("content-disposition") {
            if content_disposition.starts_with("attachment") {
                let filename = part.get_headers().get_first_value("filename").unwrap_or_else(|| "unnamed_attachment".to_string());
                let decoded_data = part.get_body_raw()?;
                attachments.push((filename, decoded_data));
            }
        }
    }
    Ok(attachments)
}

fn save_attachments_to_temp_files(attachments: &[(String, Vec<u8>)]) -> Result<Vec<path::PathBuf>, Box<dyn std::error::Error>> {
    let temp_dir = temp_dir();
    let mut file_paths = Vec::new();

    for (filename, data) in attachments {
        let temp_file_path = temp_dir.join(filename);
        let mut temp_file = File::create(&temp_file_path)?;
        file_paths.push(temp_file_path);
    }
    Ok(file_paths)
}

fn create_multipart_form(raw_email: &str,
timestamp: &str,
token: &str,
signature: &str,
file_paths: &[std::path::PathBuf],
) -> Result<multipart::Form, Box<dyn std::error::Error>> {
    let mut form = multipart::Form::new()
        .text("email", raw_email.to_string())
        .text("timestamp", timestamp.to_string())
    .text("token", token.to_string())
    .text("signature", signature.to_string());

    for (i, path) in file_paths.iter().enumerate() {
        let field_name = format!("attachment-{}", i + 1);
        form = form.file(field_name, path);
    }

    Ok(form)
}
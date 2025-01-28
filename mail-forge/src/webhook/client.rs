use std::env::temp_dir;
use std::fs::File;
use std::io::Write;
use std::path;
use chrono::Utc;
use log::{error, info};
use mailparse::MailHeaderMap;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::{multipart, Client};
use serde_json::json;
use crate::config;
use crate::webhook::utils;

pub async fn forward_to_webhook(
    webhook: &config::WebhookConfig,
    raw_email: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let (timestamp, token, signature) = generate_auth(&webhook.api_key);

    // Extract email data (subject, from, to, etc.)
    let email_data = extract_email_data(raw_email)?;

    // Parse email and extract attachments
    let attachments = extract_attachments(raw_email)?;

    // Save all attachments to temporary files
    let temp_files = save_attachments_to_temp_files(&attachments)?;

    // Create multipart form
    let form = create_multipart_form(raw_email, &email_data, &timestamp, &token, &signature, &temp_files).await?;

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
            let body = resp.text().await.unwrap_or_else(|err| {
                error!("Failed to read response body: {}", err);
                "Unable to read body".to_string()
            });

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

    let signature = utils::generate_signature(&api_key, &timestamp, &token);

    (timestamp, token, signature)
}

fn extract_attachments(raw_email: &str) -> Result<Vec<(String, Vec<u8>)>, Box<dyn std::error::Error>> {
    let parsed_mail = mailparse::parse_mail(raw_email.as_bytes()).map_err(|e| format!("Failed to parse email: {}", e))?;
    let mut attachments = Vec::new();

    parse_mime_parts(&parsed_mail, &mut attachments).map_err(|e| format!("Failed to parse MIME parts: {}", e))?;

    Ok(attachments)
}

fn parse_mime_parts(part: &mailparse::ParsedMail, attachments: &mut Vec<(String, Vec<u8>)>) -> Result<(), Box<dyn std::error::Error>> {
    for (index, subpart) in part.subparts.iter().enumerate() {
        if let Some(content_disposition) = subpart.get_headers().get_first_value("content-disposition") {
            if content_disposition.starts_with("attachment") || content_disposition.contains("filename=") {
                let filename = subpart.get_headers().get_first_value("filename")
                    .or_else(|| extract_filename_from_content_disposition(&content_disposition))
                    .unwrap_or_else(|| "unnamed_attachment".to_string());
                let decoded_data = subpart.get_body_raw().map_err(|e| format!("Failed to extract body for attachment '{}': {}", filename, e))?;

                attachments.push((filename, decoded_data));
            }
        }
        parse_mime_parts(subpart, attachments).map_err(|e| format!("Failed to parse subpart at index {}: {}", index, e))?;
    }
    Ok(())
}

fn extract_filename_from_content_disposition(content_disposition: &str) -> Option<String> {
    content_disposition.split(';').find_map(|kv| {
        let kv = kv.trim();
        if kv.starts_with("filename=") {
            Some(kv["filename=".len()..].trim_matches('"').to_string())
        } else {
            None
        }
    })
}
fn save_attachments_to_temp_files(attachments: &[(String, Vec<u8>)]) -> Result<Vec<path::PathBuf>, Box<dyn std::error::Error>> {
    let temp_dir = temp_dir();
    let mut file_paths = Vec::new();

    for (filename, data) in attachments {
        // Ensure filename is sanitized and not empty
        let mut sanitized_filename = sanitize_filename::sanitize(&filename);
        if sanitized_filename.is_empty() {
            return Err(format!("Attachment filename '{}' is invalid after sanitization.", filename).into());
        }

        // Handle duplicate filenames by appending a unique suffix;
        let mut temp_file_path = temp_dir.join(&sanitized_filename);
        let mut counter = 1;
        while temp_file_path.exists() {
            sanitized_filename = format!("{}_{}", sanitize_filename::sanitize(filename), counter);
            temp_file_path = temp_dir.join(&sanitized_filename);
            counter += 1;
        }

        // Write data to the file
        let mut temp_file = File::create(&temp_file_path)?;
        temp_file.write_all(data)?;
        file_paths.push(temp_file_path);
    }

    Ok(file_paths)
}

async fn create_multipart_form(
    raw_email: &str,
    email_data: &serde_json::Value,
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

    form = form
        .text("subject", email_data["subject"].as_str().unwrap_or_default().to_string())
        .text("from", email_data["from"].as_str().unwrap_or_default().to_string())
        .text("to", email_data["to"].as_str().unwrap_or_default().to_string())
        .text("date", email_data["date"].as_str().unwrap_or_default().to_string())
        .text("body_plain", email_data["body_plain"].as_str().unwrap_or_default().to_string())
        .text("body_html", email_data["body_html"].as_str().unwrap_or_default().to_string());

    for (i, path) in file_paths.iter().enumerate() {
        let field_name = format!("attachment-{}", i + 1);
        form = form.file(field_name.clone(), path).await?;
    }

    Ok(form)
}

fn extract_email_data(raw_email: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Parse the email
    let parsed_mail = mailparse::parse_mail(raw_email.as_bytes())?;

    // Extract headers
    let headers = parsed_mail.get_headers();
    let subject = headers.get_first_value("Subject").unwrap_or_default();
    let from = headers.get_first_value("From").unwrap_or_default();
    let to = headers.get_first_value("To").unwrap_or_default();
    let date = headers.get_first_value("Date").unwrap_or_default();

    // Extract body parts
    let mut body_plain = String::new();
    let mut body_html = String::new();


    // Check the root body part
    let content_type = parsed_mail.get_headers().get_first_value("Content-Type").unwrap_or_default();
    if content_type.starts_with("text/plain") {
        body_plain = parsed_mail.get_body()?;
    } else if content_type.starts_with("text/html") {
        body_html = parsed_mail.get_body()?;
    }

    for part in parsed_mail.subparts.iter() {
        let content_disposition = part.get_headers().get_first_value("content-disposition").unwrap_or_default();

        if content_disposition.contains("attachment") {
            continue;
        }

        let part_content_type = part.get_headers().get_first_value("Content-Type").unwrap_or_default().trim().to_string();

        if part_content_type.starts_with("text/plain") && body_plain.is_empty() {
            body_plain = part.get_body()?;
        } else if part_content_type.starts_with( "text/html") && body_html.is_empty() {
            body_html = part.get_body()?;
        }

        if !body_plain.is_empty() && !body_html.is_empty() {
            break;
        }
    }
    // Build the JSON payload
    let json_payload = json!({
        "subject": subject,
        "from": from,
        "to": to,
        "date": date,
        "body_plain": body_plain,
        "body_html": body_html,
    });

    Ok(json_payload)
}
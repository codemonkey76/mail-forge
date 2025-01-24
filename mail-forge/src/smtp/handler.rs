use std::collections::HashMap;

use log::{error, info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::webhook::client::forward_to_webhook;

const MAX_SIZE: usize = 35882577;

#[derive(Default)]
struct SessionState {
    helo: Option<String>,
    mail_from: Option<String>,
    rcpt_to: Vec<String>,
    data: Option<String>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            helo: None,
            mail_from: None,
            rcpt_to: Vec::new(),
            data: None,
        }
    }

    fn is_ready_for_data(&self) -> bool {
        self.mail_from.is_some() && !self.rcpt_to.is_empty()
    }
}

pub async fn handle_client(
    mut socket: TcpStream,
    addr: std::net::SocketAddr,
    webhook_mapping: &WebhookMapping,
) {
    info!("Accepted connection from {}", addr);

    if let Err(e) = socket
        .write_all(b"220 Mail Forge SMTP Server Ready\r\n")
        .await
    {
        error!("Failed to send greeting to {}: {}", addr, e);
        return;
    }

    let mut buffer = [0; 1024];
    let mut session_state = SessionState::new();

    loop {
        match socket.read(&mut buffer).await {
            Ok(0) => {
                info!("Connection closed by {}", addr);
                break;
            }
            Ok(n) => {
                let request = String::from_utf8_lossy(&buffer[..n]).to_string();
                info!("Received: {}", request.trim());

                // Parse and handle the command
                if let Err(e) = process_command(&mut socket, &mut session_state, &request).await {
                    info!("Closing connection with {}: {}", addr, e);
                    break;
                }
            }
            Err(e) => {
                error!("Failed to read from socket: {}", e);
                break;
            }
        }
    }
    info!("Connection with {} has been closed.", addr);
}

async fn process_command(
    socket: &mut TcpStream,
    session_state: &mut SessionState,
    request: &str,
    webhook_mapping: &WebhookMapping,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = request.trim().splitn(2, ' ');
    let command = parts.next().unwrap_or("").to_uppercase();
    let arguments = parts.next().unwrap_or("");
    info!("Command: {}", command);
    info!("Arguments: {}", arguments);

    match command.as_str() {
        "HELO" => handle_helo(socket, session_state, arguments).await,
        "EHLO" => handle_ehlo(socket, session_state, arguments).await,
        "MAIL" if arguments.to_uppercase().starts_with("FROM:") => {
            handle_mail_from(socket, session_state, arguments).await
        }
        "RCPT" if arguments.to_uppercase().starts_with("TO:") => {
            handle_rcpt_to(socket, session_state, arguments).await
        }
        "DATA" => {
            if session_state.is_ready_for_data() {
                handle_data(socket, session_state, webhook_mapping).await
            } else {
                socket
                    .write_all(b"503 5.5.1 Error: need RCPT command\r\n")
                    .await?;
                Ok(())
            }
        }
        "RSET" => handle_rset(socket, session_state).await,
        "NOOP" => handle_noop(socket).await,
        "QUIT" => handle_quit(socket).await,
        _ => {
            socket
                .write_all(b"500 Syntax error, command unrecognized\r\n")
                .await?;
            Ok(())
        }
    }
}

async fn handle_helo(
    socket: &mut TcpStream,
    state: &mut SessionState,
    arguments: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    state.helo = Some(arguments.to_string());

    let response = format!(
        "250-Mail FORGE SMTP Server Ready\r\n\
        250-Size {}\r\n\
        250-8BITMIME\r\n\
        250 HELP\r\n",
        MAX_SIZE
    );

    socket.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn handle_ehlo(
    socket: &mut TcpStream,
    state: &mut SessionState,
    arguments: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    state.helo = Some(arguments.to_string());
    socket
        .write_all(b"250-Mail Forge SMTP Server REady\r\n250-SIZE 35882577\r\n250 HELP\r\n")
        .await?;
    Ok(())
}

async fn handle_mail_from(
    socket: &mut TcpStream,
    state: &mut SessionState,
    arguments: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !arguments.to_uppercase().starts_with("FROM:") {
        socket
            .write_all(b"501 5.5.2 Syntax error in parameters or arguments\r\n")
            .await?;
        return Ok(());
    }

    let email_start = arguments.find(':').unwrap_or(0) + 1;
    let email = arguments[email_start..]
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>');
    if email.is_empty() {
        socket
            .write_all(b"501 5.5.2 Syntax error: Empty email address\r\n")
            .await?;
        return Ok(());
    }

    state.mail_from = Some(email.to_string());
    socket.write_all(b"250 2.1.0 OK\r\n").await?;
    Ok(())
}
async fn handle_rcpt_to(
    socket: &mut TcpStream,
    state: &mut SessionState,
    arguments: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !arguments.to_uppercase().starts_with("TO:") {
        socket
            .write_all(b"501 5.5.2 Syntax error in parameters or arguments\r\n")
            .await?;
        return Ok(());
    }

    let email_start = arguments.find(':').unwrap_or(0) + 1;
    let email = arguments[email_start..]
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>');

    if email.is_empty() {
        socket
            .write_all(b"501 5.5.2 Syntax error: Empty recipient address\r\n")
            .await?;
        return Ok(());
    }

    state.rcpt_to.push(email.to_string());
    socket.write_all(b"250 2.5.1 OK\r\n").await?;
    Ok(())
}

pub struct WebhookMapping {
    map: HashMap<String, String>,
}

impl WebhookMapping {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn add_mapping(&mut self, recipient: &str, webhook: &str) {
        self.map.insert(recipient.to_string(), webhook.to_string());
    }

    fn get_webhook(&self, recipient: &str) -> Option<&String> {
        self.map.get(recipient)
    }
}

pub fn load_webhook_mapping() -> WebhookMapping {
    let mut mapping = WebhookMapping::new();
    mapping.add_mapping("recipient1@example.com", "https://webhook.site/1");
    mapping.add_mapping("recipient1@example.com", "https://webhook.site/1");
    mapping.add_mapping("recipient1@example.com", "https://webhook.site/1");
    mapping.add_mapping("recipient1@example.com", "https://webhook.site/1");
    mapping
}

async fn handle_data(
    socket: &mut TcpStream,
    state: &mut SessionState,
    webhook_mapping: &WebhookMapping,
) -> Result<(), Box<dyn std::error::Error>> {
    socket
        .write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n")
        .await?;

    let mut buffer = [0; 1024];
    let mut data = String::new();
    let mut total_size = 0;

    let mut last_few_chars = String::new();

    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            break;
        }

        total_size += n;
        if total_size > MAX_SIZE {
            socket
                .write_all(b"552 Message size exceeds maximum permitted\r\n")
                .await?;
            return Err("Message exceeeds maximum size.".into());
        }

        let chunk = String::from_utf8_lossy(&buffer[..n]);
        last_few_chars.push_str(&chunk);
        data.push_str(&chunk);

        if last_few_chars.ends_with("\r\n.\r\n") {
            data.truncate(data.len() - 5);
            info!("End of data detected. Total size: {} bytes", total_size);
            break;
        }

        if last_few_chars.len() > 10 {
            last_few_chars = last_few_chars[last_few_chars.len() - 10..].to_string();
        }
        info!("Received data chunk: {} bytes", chunk.len());
        info!("Current total size: {} bytes", total_size);
    }
    state.data = Some(data.clone());
    socket.write_all(b"250 OK: Message received\r\n").await?;

    for recipient in &state.rcpt_to {
        if let Some(webhook) = webhook_mapping.get_webhook(recipient) {
            if let Err(e) = forward_to_webhook(webhook, &data).await {
                error!("Failed to forward email to {}: {}", webhook, e);
            } else {
                info!(
                    "Email forwarded to webhook {} for recipient {}",
                    webhook, recipient
                );
            }
        } else {
            error!("No webhook mapping found for recipient {}", recipient);
        }
    }
    Ok(())
}
async fn handle_rset(
    socket: &mut TcpStream,
    state: &mut SessionState,
) -> Result<(), Box<dyn std::error::Error>> {
    *state = SessionState::new();
    socket.write_all(b"250 OK\r\n").await?;
    Ok(())
}
async fn handle_noop(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    socket.write_all(b"250 OK\r\n").await?;
    Ok(())
}
async fn handle_quit(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    socket.write_all(b"221 Bye\r\n").await?;
    Err("QUIT received. Closing connection.")?
}

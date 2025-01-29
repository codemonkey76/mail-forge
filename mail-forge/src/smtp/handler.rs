use crate::config;
use crate::smtp::stream::StreamType;
use crate::webhook::client::forward_to_webhook;
use crate::webhook::mapping::get_webhook_for_recipient;
use chrono::Utc;
use log::{error, info};
use rustls::ServerConfig;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::TlsAcceptor;

#[derive(Default)]
struct SessionState {
    helo: Option<String>,
    mail_from: Option<String>,
    rcpt_to: Vec<String>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            helo: None,
            mail_from: None,
            rcpt_to: Vec::new(),
        }
    }

    fn is_ready_for_data(&self) -> bool {
        self.mail_from.is_some() && !self.rcpt_to.is_empty()
    }
}

pub async fn handle_client(
    mut socket: TcpStream,
    tls_config: Arc<ServerConfig>,
    addr: std::net::SocketAddr,
    config: Arc<config::Config>,
) {
    info!("Accepted connection from {}", addr);

    // Send the initial SMTP greeting
    if let Err(e) = socket
        .write_all(
            format!(
                "220 {} Mail Forge SMTP Server Ready\r\n",
                config.server.hostname
            )
            .as_bytes(),
        )
        .await
    {
        error!("Failed to send greeting to {}: {}", addr, e);
        return;
    }

    // Initialize the session state
    let mut session_state = SessionState::new();

    // Process commands using process_commands
    let stream = StreamType::Plain(BufReader::new(socket));
    if let Err(e) = process_commands(stream, &mut session_state, config, tls_config).await {
        error!("Error processing commands for {}: {}", addr, e);
    }

    info!("Connection with {} has been closed.", addr);
}

async fn process_commands<S>(
    mut stream: StreamType<S>,
    state: &mut SessionState,
    config: Arc<config::Config>,
    tls_config: Arc<ServerConfig>,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut line = String::new();

    loop {
        while stream.read_line(&mut line).await? != 0 {
            let request = line.trim().to_string();
            line.clear(); // Clear the buffer

            let mut parts = request.splitn(2, ' ');
            let command = parts.next().unwrap_or("").to_uppercase();
            let arguments = parts.next().unwrap_or("");

            if command.is_empty() {
                stream
                    .write_all(b"500 Syntax error, command unrecognized\r\n")
                    .await?;
                continue;
            }

            match command.as_str() {
                "HELO" => handle_helo(&mut stream, state, config.clone(), arguments).await?,
                "EHLO" => handle_ehlo(&mut stream, state, config.clone(), arguments).await?,

                "RSET" => handle_rset(&mut stream, state).await?,
                "NOOP" => handle_noop(&mut stream).await?,
                "DATA" => handle_data(&mut stream, state, config.clone()).await?,
                "MAIL" if arguments.to_uppercase().starts_with("FROM:") => {
                    handle_mail_from(&mut stream, state, arguments).await?
                }
                "RCPT" if arguments.to_uppercase().starts_with("TO:") => {
                    handle_rcpt_to(&mut stream, state, config.clone(), arguments).await?
                }
                "QUIT" => {
                    handle_quit(&mut stream).await?;
                    break;
                }
                "STARTTLS" => {
                    stream = handle_starttls(stream, tls_config.clone()).await?;
                    continue;
                }
                _ => {
                    stream
                        .write_all(b"500 Syntax error, command Unrecognized\r\n")
                        .await?
                }
            }
        }
    }
}

async fn handle_starttls<S>(
    mut stream: StreamType<S>,
    tls_config: Arc<ServerConfig>,
) -> Result<StreamType<S>, Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // Ensure TLS isn't already active
    if matches!(stream, StreamType::Tls(_)) {
        stream.write_all(b"503 TLS already active\r\n").await?;
        return Err("TLS already active".into());
    }

    stream.write_all(b"220 Ready to start TLS\r\n").await?;

    let inner_stream = match stream {
        StreamType::Plain(inner) => inner.into_inner(),
        _ => return Err("Stream must be in plain variant".into()),
    };

    let tls_stream = TlsAcceptor::from(tls_config).accept(inner_stream).await?;

    return Ok(StreamType::Tls(BufReader::new(tls_stream)));
}

async fn handle_helo<S>(
    stream: &mut StreamType<S>,
    state: &mut SessionState,
    config: Arc<config::Config>,
    arguments: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    state.helo = Some(arguments.to_string());

    let response = format!(
        "250 {} Mail FORGE ESMTP Server Ready\r\n",
        config.server.hostname
    );

    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn handle_ehlo<S>(
    stream: &mut StreamType<S>,
    state: &mut SessionState,
    config: Arc<config::Config>,
    arguments: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    state.helo = Some(arguments.to_string());
    stream
        .write_all(
            format!(
                "250-{} Mail Forge ESMTP Server Ready\r\n\
                    250-STARTTLS\r\n\
                    250 SIZE {}\r\n",
                config.server.hostname, config.server.max_size,
            )
            .as_bytes(),
        )
        .await?;
    Ok(())
}

async fn handle_mail_from<S>(
    stream: &mut StreamType<S>,
    state: &mut SessionState,
    arguments: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if !arguments.to_uppercase().starts_with("FROM:") {
        stream
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
        stream
            .write_all(b"501 5.5.2 Syntax error: Empty email address\r\n")
            .await?;
        return Ok(());
    }

    state.mail_from = Some(email.to_string());
    stream.write_all(b"250 2.1.0 OK\r\n").await?;
    Ok(())
}

async fn handle_rcpt_to<S>(
    stream: &mut StreamType<S>,
    state: &mut SessionState,
    config: Arc<config::Config>,
    arguments: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if !arguments.to_uppercase().starts_with("TO:") {
        stream
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
        stream
            .write_all(b"501 5.5.2 Syntax error: Empty recipient address\r\n")
            .await?;
        return Ok(());
    }

    if get_webhook_for_recipient(email, &config.webhooks).is_some() {
        state.rcpt_to.push(email.to_string());
        info!("Adding recipient: {}", email);
        stream.write_all(b"250 2.1.5 Recipient OK\r\n").await?;
    } else {
        info!("Skipping recipient: {}", email);
        stream.write_all(b"550 5.7.1 Unable to relay\r\n").await?;
    }
    Ok(())
}

async fn handle_data<S>(
    stream: &mut StreamType<S>,
    state: &mut SessionState,
    config: Arc<config::Config>,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if !state.is_ready_for_data() {
        stream
            .write_all(b"503 Bad sequence of commands\r\n")
            .await?;
        return Err("Session is not ready to accept DATA".into());
    }
    stream
        .write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n")
        .await?;

    let mut email_data = String::new();
    let mut total_size = 0;

    loop {
        let mut line = String::new();
        let bytes_read = stream.read_line(&mut line).await?;

        if bytes_read == 0 {
            return Err("Client disconnected during DATA phase".into());
        }

        total_size += bytes_read;

        if total_size > config.server.max_size {
            stream
                .write_all(b"552 Message size exceeds maximum permitted\r\n")
                .await?;
            return Err("Email data size exceeded maximum limit.".into());
        }

        if line == ".\r\n" {
            save_email(&email_data).expect("Failed to save email to file");
            break;
        }

        email_data.push_str(&line);
    }

    let mut successfully_forwarded = false;

    for recipient in &state.rcpt_to {
        if let Some(webhook) = get_webhook_for_recipient(recipient, &config.webhooks) {
            match forward_to_webhook(recipient, webhook, &email_data).await {
                Ok(_) => {
                    info!(
                        "Email successfully forwarded to webhook {} for recipient {}",
                        webhook.url, recipient
                    );
                    successfully_forwarded = true;
                }
                Err(e) => {
                    error!(
                        "Failed to forward email to webhook {} for recipient {}: {}",
                        webhook.url, recipient, e
                    );
                }
            }
        } else {
            error!("No webhook mapping found for recipient: {}", recipient);
        }
    }

    if successfully_forwarded {
        stream.write_all(b"250 OK\r\n").await?;
    } else {
        stream
            .write_all(b"554 Failed to process email for all recipients.\r\n")
            .await?;
    }
    Ok(())
}

fn save_email(raw_email: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir_path = "/var/log/mail-forge/emails";
    std::fs::create_dir_all(dir_path)?;

    let file_path = format!("{}/{}.eml", dir_path, Utc::now().timestamp());
    info!("Saving email to: {}", file_path);
    let mut file = File::create(&file_path)?;
    file.write_all(raw_email.as_bytes())?;
    Ok(())
}

async fn handle_rset<S>(
    stream: &mut StreamType<S>,
    state: &mut SessionState,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    *state = SessionState::new();
    stream.write_all(b"250 OK\r\n").await?;
    Ok(())
}

async fn handle_noop<S>(stream: &mut StreamType<S>) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    stream.write_all(b"250 OK\r\n").await?;
    Ok(())
}

async fn handle_quit<S>(stream: &mut StreamType<S>) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    stream.write_all(b"221 Bye\r\n").await?;
    Err("QUIT received. Closing connection.")?
}

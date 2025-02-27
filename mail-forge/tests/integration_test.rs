
#[cfg(test)]
mod tests {

    use std::fs;
    use mail_forge::{config, webhook};
    use mail_forge::webhook::mapping::get_webhook_for_recipient;

    #[tokio::test]
    async fn test_forward_multiple_emails() {
        for entry in fs::read_dir("tests/emails").expect("Failed to read email test directory") {
            let path = entry.expect("Failed to read entry").path();
            let raw_email = fs::read_to_string(&path).expect("Failed to read email file");
            let config = config::Config::load("../config.toml").expect("Failed to parse config.toml");

            let webhook = get_webhook_for_recipient("shane@textify.asgcom.net", &config.webhooks).expect("Failed to get webhook");

            // Assert that the webhook forward succeeds
            match webhook::client::forward_to_webhook(&webhook, &raw_email).await {
                Ok(_) => println!("Forwarding succeeded for email at: {:?}", path),
                Err(e) => {
                    panic!("Forwarding failed for email at {:?}: {}", path, e);
                }
            }
        }
    }
}
use std::collections::HashMap;
use crate::config;

pub fn get_webhook_for_recipient<'a>(
    recipient: &str,
    webhook_mapping: &'a HashMap<String, config::WebhookConfig>,
) -> Option<&'a config::WebhookConfig> {
    if let Some(webhook) = webhook_mapping.get(recipient) {
        return Some(webhook);
    }

    for (pattern, webhook) in webhook_mapping {
        if pattern.starts_with("*@") {
            let domain = &pattern[2..];
            if recipient.ends_with(domain) {
                return Some(webhook);
            }
        }
    }

    None
}

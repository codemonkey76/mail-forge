use std::collections::HashMap;

use crate::config::structs::WebhookConfig;

//#[derive(Clone)]
//pub struct WebhookMapping {
//    map: HashMap<String, String>,
//}
//
//impl WebhookMapping {
//    fn new() -> Self {
//        Self {
//            map: HashMap::new(),
//        }
//    }
//
//    pub fn add_mapping(&mut self, recipient: &str, webhook: &str) {
//        self.map.insert(recipient.to_string(), webhook.to_string());
//    }
//
//    pub fn get_webhook(&self, recipient: &str) -> Option<&String> {
//        self.map.get(recipient)
//    }
//}
//
//pub fn load_webhook_mapping() -> WebhookMapping {
//    let mut mapping = WebhookMapping::new();
//    mapping.add_mapping("recipient1@example.com", "https://webhook.site/1");
//    mapping.add_mapping("recipient1@example.com", "https://webhook.site/1");
//    mapping.add_mapping("recipient1@example.com", "https://webhook.site/1");
//    mapping.add_mapping("recipient1@example.com", "https://webhook.site/1");
//    mapping
//}

pub fn get_webhook_for_recipient<'a>(
    recipient: &str,
    webhook_mapping: &'a HashMap<String, WebhookConfig>,
) -> Option<&'a WebhookConfig> {
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

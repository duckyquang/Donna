use crate::config::Config;

pub async fn send(cfg: &Config, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(token), Some(phone_id), Some(to)) = (
        &cfg.whatsapp_token,
        &cfg.whatsapp_phone_id,
        &cfg.my_whatsapp,
    ) {
        send_whatsapp(token, phone_id, to, message).await?;
    } else if let (Some(token), Some(chat_id)) = (&cfg.telegram_token, &cfg.telegram_chat_id) {
        send_telegram(token, chat_id, message).await?;
    }
    Ok(())
}

async fn send_whatsapp(token: &str, phone_id: &str, to: &str, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let body = serde_json::json!({
        "messaging_product": "whatsapp",
        "to": to,
        "type": "text",
        "text": { "body": text }
    });
    reqwest::Client::new()
        .post(format!("https://graph.facebook.com/v18.0/{phone_id}/messages"))
        .bearer_auth(token)
        .json(&body)
        .send().await?;
    Ok(())
}

async fn send_telegram(token: &str, chat_id: &str, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let body = serde_json::json!({ "chat_id": chat_id, "text": text, "parse_mode": "Markdown" });
    reqwest::Client::new().post(&url).json(&body).send().await?;
    Ok(())
}

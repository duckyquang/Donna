pub struct Config {
    pub whatsapp_token: Option<String>,
    pub whatsapp_phone_id: Option<String>,
    pub my_whatsapp: Option<String>,
    pub telegram_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_refresh_token: Option<String>,
    pub ai_provider: String,
    pub ai_key: Option<String>,
    pub ai_model: String,
    pub ollama_url: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub news_hour: u32,
    pub briefing_hour: u32,
}

impl Config {
    pub fn from_env() -> Self {
        let e = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Config {
            whatsapp_token: e("DONNA_WHATSAPP_TOKEN"),
            whatsapp_phone_id: e("DONNA_WHATSAPP_PHONE_ID"),
            my_whatsapp: e("DONNA_MY_WHATSAPP"),
            telegram_token: e("DONNA_TELEGRAM_TOKEN"),
            telegram_chat_id: e("DONNA_TELEGRAM_CHAT_ID"),
            google_client_id: e("DONNA_GOOGLE_CLIENT_ID"),
            google_client_secret: e("DONNA_GOOGLE_CLIENT_SECRET"),
            google_refresh_token: e("DONNA_GOOGLE_REFRESH_TOKEN"),
            ai_provider: std::env::var("DONNA_AI_PROVIDER").unwrap_or_else(|_| "none".into()),
            ai_key: e("DONNA_AI_KEY"),
            ai_model: std::env::var("DONNA_AI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into()),
            ollama_url: e("DONNA_OLLAMA_URL"),
            lat: e("DONNA_LAT").and_then(|v| v.parse().ok()),
            lon: e("DONNA_LON").and_then(|v| v.parse().ok()),
            news_hour: std::env::var("DONNA_NEWS_HOUR").ok().and_then(|v| v.parse().ok()).unwrap_or(9),
            briefing_hour: std::env::var("DONNA_BRIEFING_HOUR").ok().and_then(|v| v.parse().ok()).unwrap_or(8),
        }
    }

    #[allow(dead_code)]
    pub fn has_messaging(&self) -> bool {
        self.whatsapp_token.is_some() || self.telegram_token.is_some()
    }
}

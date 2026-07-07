//! Voice endpoints: `/voice/transcribe` (Whisper) and `/voice/speak` (TTS). Both live
//! INSIDE the bearer-auth layer (registered in `lib.rs` before `.layer(require_bearer)`) —
//! unlike the WhatsApp webhook, there's no other credential guarding these, so they must
//! inherit our own auth.

use axum::{
    extract::{Multipart, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use donna_core::audio;
use donna_core::secrets;
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({"error": msg.into()}))).into_response()
}

/// Fetch the OpenAI key, or a ready-made 400 response if none is configured.
fn require_openai_key() -> Result<String, Response> {
    match secrets::get_api_key("openai") {
        Ok(Some(key)) => Ok(key),
        Ok(None) => Err(err(StatusCode::BAD_REQUEST, "OpenAI key required for voice")),
        Err(e) => Err(err(StatusCode::BAD_REQUEST, e.to_string())),
    }
}

/// POST `/voice/transcribe` — `multipart/form-data` with a `file` part (audio bytes).
pub async fn transcribe(State(_st): State<AppState>, mut mp: Multipart) -> Response {
    let mut file: Option<(String, Vec<u8>)> = None;
    while let Ok(Some(field)) = mp.next_field().await {
        if field.name() == Some("file") {
            let filename = field.file_name().unwrap_or("audio").to_string();
            let Ok(bytes) = field.bytes().await else {
                return err(StatusCode::BAD_REQUEST, "could not read file part");
            };
            file = Some((filename, bytes.to_vec()));
            break;
        }
    }
    let Some((filename, bytes)) = file else {
        return err(StatusCode::BAD_REQUEST, "missing file");
    };

    let key = match require_openai_key() {
        Ok(k) => k,
        Err(resp) => return resp,
    };

    match audio::transcribe(&key, audio::DEFAULT_TRANSCRIBE_MODEL, bytes, &filename).await {
        Ok(text) => Json(json!({"transcript": text})).into_response(),
        Err(e) => err(StatusCode::BAD_GATEWAY, e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct SpeakReq {
    text: String,
    voice: Option<String>,
}

/// POST `/voice/speak` — `{text, voice?}` → `audio/mpeg` bytes.
pub async fn speak(State(_st): State<AppState>, Json(req): Json<SpeakReq>) -> Response {
    let voice = req
        .voice
        .filter(|v| audio::is_valid_voice(v))
        .unwrap_or_else(|| audio::DEFAULT_TTS_VOICE.to_string());

    let key = match require_openai_key() {
        Ok(k) => k,
        Err(resp) => return resp,
    };

    match audio::synthesize(&key, audio::DEFAULT_TTS_MODEL, &voice, &req.text, "mp3").await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "audio/mpeg")], bytes).into_response(),
        Err(e) => err(StatusCode::BAD_GATEWAY, e.to_string()),
    }
}

//! OpenAI audio: Whisper transcription and TTS synthesis.

use reqwest::multipart::{Form, Part};

use crate::error::{Error, Result};
use crate::providers::http_client;

pub const DEFAULT_TTS_VOICE: &str = "nova";
pub const DEFAULT_TTS_MODEL: &str = "tts-1";
pub const DEFAULT_TRANSCRIBE_MODEL: &str = "whisper-1";

const VALID_VOICES: &[&str] = &[
    "nova", "shimmer", "coral", "sage", "ballad", "alloy", "echo", "fable", "onyx", "ash", "verse",
];

pub fn is_valid_voice(v: &str) -> bool {
    VALID_VOICES.contains(&v)
}

fn guess_mime(filename: &str) -> &'static str {
    match filename.rsplit('.').next().unwrap_or_default() {
        "webm" => "audio/webm",
        "ogg" => "audio/ogg",
        "mp3" => "audio/mpeg",
        "m4a" | "mp4" => "audio/mp4",
        "wav" => "audio/wav",
        _ => "audio/mpeg",
    }
}

fn tts_body(model: &str, voice: &str, input: &str, format: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "voice": voice,
        "input": input,
        "response_format": format,
    })
}

/// Transcribe audio bytes via OpenAI Whisper. Returns the transcript text.
pub async fn transcribe(api_key: &str, model: &str, audio: Vec<u8>, filename: &str) -> Result<String> {
    let form = Form::new().text("model", model.to_string()).part(
        "file",
        Part::bytes(audio)
            .file_name(filename.to_string())
            .mime_str(guess_mime(filename))?,
    );
    let resp = http_client()
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!(
            "OpenAI error ({status}): {detail}"
        )));
    }

    let v = resp.json::<serde_json::Value>().await?;
    Ok(v["text"].as_str().unwrap_or_default().to_string())
}

/// Synthesize speech via OpenAI TTS. Returns the raw audio bytes.
pub async fn synthesize(
    api_key: &str,
    model: &str,
    voice: &str,
    input: &str,
    format: &str,
) -> Result<Vec<u8>> {
    let body = tts_body(model, voice, input, format);
    let resp = http_client()
        .post("https://api.openai.com/v1/audio/speech")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!(
            "OpenAI error ({status}): {detail}"
        )));
    }

    Ok(resp.bytes().await?.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_validation() {
        assert!(is_valid_voice("nova"));
        assert!(is_valid_voice("shimmer"));
        assert!(!is_valid_voice("bogus"));
        assert_eq!(DEFAULT_TTS_VOICE, "nova");
    }

    #[test]
    fn tts_body_shape() {
        let b = tts_body("tts-1", "nova", "hello", "opus");
        assert_eq!(b["model"], "tts-1");
        assert_eq!(b["voice"], "nova");
        assert_eq!(b["input"], "hello");
        assert_eq!(b["response_format"], "opus");
    }

    #[test]
    fn mime_by_extension() {
        assert_eq!(guess_mime("clip.webm"), "audio/webm");
        assert_eq!(guess_mime("note.ogg"), "audio/ogg");
        assert_eq!(guess_mime("x.m4a"), "audio/mp4");
        assert_eq!(guess_mime("weird"), "audio/mpeg");
    }
}

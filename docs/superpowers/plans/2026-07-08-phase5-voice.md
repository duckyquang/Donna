# Phase 5: Voice — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Talk to Donna and hear her back. Desktop push-to-talk (speak → she transcribes, answers, and replies aloud in a female voice) and WhatsApp voice notes (send her a voice memo, she transcribes it, runs the brain, and replies with her own voice note).

**Architecture:** A new `donna-core::audio` module wraps two OpenAI endpoints — Whisper transcription (`/v1/audio/transcriptions`, multipart) and TTS (`/v1/audio/speech`, binary out). WhatsApp voice: the webhook gains an `audio` arm that downloads the media (2-step Meta Graph fetch), transcribes, runs the existing brain over the rolling session, and replies with a synthesized voice note (text fallback). Desktop voice: the webview records mic audio with the browser `MediaRecorder` API, POSTs it to two new bearer-authed server routes (`/voice/transcribe`, `/voice/speak`); transcription feeds the normal chat flow (streamed reply over the existing WS), and TTS of the final reply plays back when "speak replies" is on.

**Tech Stack:** existing + the `multipart` reqwest feature (already-present crate, one feature flag). Browser `MediaRecorder`/`getUserMedia` in WKWebView; macOS mic entitlement.

## Global Constraints

- Spec §6 (voice surfaces): **desktop push-to-talk + WhatsApp voice notes**. No phone calls (deferred).
- **Commit AND push after every task.** Branch `feat/phase-5-projects-discord-proactive`, no PRs.
- **Voice requires an OpenAI key**, independent of the chat provider. Every voice entry point fetches `secrets::get_api_key("openai")` and fails gracefully (clear error / disabled UI) when absent — it must NOT assume the chat provider is OpenAI.
- Default TTS voice **`nova`** (female); configurable via setting `tts_voice` (allow nova/shimmer/coral/sage/ballad/alloy/echo/fable/onyx/ash/verse; validate against that set). Default transcription model `whisper-1`, TTS model `tts-1`.
- TTS output format: **`opus`** (Ogg/Opus container) for WhatsApp voice notes; **`mp3`** for desktop playback. `audio::synthesize` takes the format as a parameter.
- Provider fns take `api_key: &str` (matching `providers.rs` convention); the caller (ops/server) fetches the key. `http_client()` in providers.rs becomes `pub(crate)` so `audio.rs` reuses the shared pool.
- Reuse the brain verbatim: transcribed text goes through the SAME `send_chat`/rolling-session path as typed text — voice is an input/output skin, not a new brain.
- WhatsApp handlers stay best-effort (`let _ =`, return Ok) so the webhook never 500s; a TTS/upload failure falls back to a text reply.
- Desktop mic is the one piece needing the owner's machine to fully verify (WKWebView mic permission prompt) — like WhatsApp needed Meta setup. Plan notes the manual check; automated tests cover the server/core paths.

---

### Task 1: audio module — transcribe + synthesize

**Files:**
- Modify: `Cargo.toml` (root — add `multipart` to reqwest features), `crates/donna-core/src/providers.rs` (`pub(crate) fn http_client`), `crates/donna-core/src/lib.rs` (`pub mod audio;`)
- Create: `crates/donna-core/src/audio.rs`
- Test: inline in audio.rs

**Interfaces:**
- `Cargo.toml:9` → `reqwest = { version = "0.12", features = ["json", "stream", "multipart"] }`.
- providers.rs: change `fn http_client()` (~line 24) to `pub(crate) fn http_client()`.
- audio.rs:
  - `pub const DEFAULT_TTS_VOICE: &str = "nova";` `pub const DEFAULT_TTS_MODEL: &str = "tts-1";` `pub const DEFAULT_TRANSCRIBE_MODEL: &str = "whisper-1";`
  - `pub fn is_valid_voice(v: &str) -> bool` — membership in the allowed set (nova/shimmer/coral/sage/ballad/alloy/echo/fable/onyx/ash/verse).
  - `pub async fn transcribe(api_key: &str, model: &str, audio: Vec<u8>, filename: &str) -> Result<String>` — multipart POST `https://api.openai.com/v1/audio/transcriptions`: `Form::new().text("model", model).part("file", Part::bytes(audio).file_name(filename).mime_str(guess_mime(filename))?)`; standard OpenAI status/error branch (mirror providers.rs verbatim); `resp.json::<Value>()`, return `["text"].as_str().unwrap_or_default().to_string()`.
  - `pub async fn synthesize(api_key: &str, model: &str, voice: &str, input: &str, format: &str) -> Result<Vec<u8>>` — JSON POST `https://api.openai.com/v1/audio/speech` body `{model, voice, input, response_format: format}`; status/error branch (error body is JSON — read `.text()` on the error path only); success → `resp.bytes().await?.to_vec()`.
  - pure `fn guess_mime(filename: &str) -> &'static str` — by extension: webm→audio/webm, ogg→audio/ogg, mp3→audio/mpeg, m4a/mp4→audio/mp4, wav→audio/wav, default audio/mpeg.
  - pure `fn tts_body(model, voice, input, format) -> serde_json::Value` (testable).

- [ ] **Step 1: Failing tests** (pure builders + validation — NO network):

```rust
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
```

- [ ] **Step 2: RED** — `cargo test -p donna-core audio` → FAIL. **Step 3: Implement** (mirror the OpenAI request/error pattern from providers.rs `stream_openai`/`openai_agent_step` exactly). **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace`, zero new warnings (the multipart feature must compile).
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add audio module: OpenAI Whisper transcription and TTS synthesis

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 2: WhatsApp media + inbound voice notes

**Files:**
- Modify: `crates/donna-core/src/integrations/whatsapp.rs` (media helpers), `crates/donna-core/src/ops.rs` (whatsapp_handle_audio), `donna-server/src/webhook.rs` (audio arm + Audio struct)
- Test: inline in whatsapp.rs (pure body/URL builders) + ops.rs

**Interfaces:**
- whatsapp.rs (reuse the existing `access_token()`/`phone_number_id()` helpers + Graph base `https://graph.facebook.com/v19.0`):
  - `pub async fn download_media(media_id: &str) -> Result<Vec<u8>>` — 2 steps: GET `/{media_id}` with `.bearer_auth(access_token()?)` → `["url"].as_str()`; then GET that url ALSO with `.bearer_auth(access_token()?)` (Meta CDN requires the token) → `.bytes().await?.to_vec()`. Standard status/error branch on both.
  - `pub async fn upload_media(bytes: Vec<u8>, mime: &str) -> Result<String>` — multipart POST `/{phone_id}/media`: `Form::new().text("messaging_product","whatsapp").text("type", mime.to_string()).part("file", Part::bytes(bytes).file_name("donna.ogg").mime_str(mime)?)`; `.bearer_auth(access_token()?)`; returns `["id"]`.
  - `pub async fn send_voice_note(to: &str, audio: Vec<u8>) -> Result<()>` — `let id = upload_media(audio, "audio/ogg").await?;` then POST `/{phone_id}/messages` body `{messaging_product:"whatsapp", to: to.trim_start_matches('+'), type:"audio", audio:{id}}` with the same status/error style as `send_message`.
- ops.rs `pub async fn whatsapp_handle_audio(db: &Db, media_id: &str) -> Result<()>`:
  1. `let bytes = whatsapp::download_media(media_id).await?;`
  2. `let key = secrets::get_api_key("openai")?` — if None → best-effort `whatsapp::send_message(my_number, "I can't transcribe voice notes without an OpenAI key set.")` and return Ok.
  3. `let text = audio::transcribe(&key, audio::DEFAULT_TRANSCRIBE_MODEL, bytes, "note.ogg").await` — on Err → best-effort text reply "I couldn't understand that voice note." + Ok.
  4. Reuse the brain by calling `whatsapp_handle_text(db, &text).await` — BUT that replies with a TEXT message. Phase 2 requirement is a voice reply "in kind". So instead: replicate whatsapp_handle_text's session+brain body to get the assistant reply string, then reply with a VOICE NOTE (Task 3 adds send_voice_note reply); for THIS task, reply as TEXT (call whatsapp_handle_text with the transcript). `// ponytail: text reply now; voice reply lands in Task 3.` Prepend a subtle marker so the user sees the transcript: actually just let the brain reply to the transcript as if typed — cleanest.
- webhook.rs: add `audio: Option<Audio>` to `Message` (bare Option, matches text/interactive), `#[derive(Deserialize)] struct Audio { id: String }`; add an `"audio" =>` arm in `dispatch` (between interactive and the `_ =>` catch-all): extract `msg.audio.map(|a| a.id)`; Some → `tokio::spawn(async move { let _ = ops::whatsapp_handle_audio(&db, &id).await; })`; None → `polite_reply(db)` (malformed).

- [ ] **Step 1: Failing tests**

```rust
// whatsapp.rs — test the pure send-body builder (extract like send_message likely has, or add a small pure fn)
#[test]
fn voice_note_message_body() {
    let b = voice_note_body("15550100", "MEDIA123");
    assert_eq!(b["type"], "audio");
    assert_eq!(b["audio"]["id"], "MEDIA123");
    assert_eq!(b["to"], "15550100");
}
```
```rust
// ops.rs — no-key path: whatsapp_handle_audio with no OpenAI key configured returns Ok without panicking
// (download will fail without creds, so test the KEY-ABSENT branch by ordering the key check BEFORE download,
//  OR assert the fn returns Ok(()) end-to-end with no creds — it must swallow all errors).
#[tokio::test]
async fn whatsapp_handle_audio_no_creds_is_ok() {
    let db = test_db();
    assert!(whatsapp_handle_audio(&db, "nonexistent-media").await.is_ok());
}
```
(Extract `fn voice_note_body(to, media_id) -> Value` in whatsapp.rs so send_voice_note and the test share it. Order the OpenAI-key check appropriately so the no-creds path is deterministic and network-free — check the key first, then download; if download needs to come first for real flow, the test asserts the whole fn still returns Ok with no creds because every error is swallowed.)

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test --workspace && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add WhatsApp media download/upload and inbound voice-note transcription

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 3: WhatsApp voice replies (TTS out)

**Files:**
- Modify: `crates/donna-core/src/ops.rs` (whatsapp_handle_audio replies with voice + text fallback)
- Test: inline (the reply-modality decision is the testable pure part)

**Interfaces:**
- Refactor `whatsapp_handle_audio` so after it has the transcript, it runs the brain over the rolling session ITSELF (don't delegate to whatsapp_handle_text, which sends text) — mirror whatsapp_handle_text's body: `whatsapp_session_conversation` → `add_message(user, transcript)` → capture last id → `send_chat(db, conv, &on_event)` → read the new assistant message.
- Reply: attempt a voice note — `audio::synthesize(&key, DEFAULT_TTS_MODEL, tts_voice_setting(db), &reply, "opus")` → `whatsapp::send_voice_note(my_number, ogg)`. On ANY failure (synth or upload/send) → fall back to `whatsapp::send_message(my_number, &reply)`. Both best-effort; return Ok regardless.
- `tts_voice_setting(db) -> String` helper (setting `tts_voice`, validated via `audio::is_valid_voice`, else DEFAULT_TTS_VOICE) — put in ops.rs; Task 5 also reads it.
- Pure `fn reply_is_voice_suitable(reply: &str) -> bool` — false when empty or very long (> 2000 chars → text, since a multi-minute TTS is bad UX); true otherwise. Gate the voice attempt on it.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn voice_reply_suitability() {
    assert!(reply_is_voice_suitable("Sure, your meeting is at 3pm."));
    assert!(!reply_is_voice_suitable(""));
    assert!(!reply_is_voice_suitable(&"x".repeat(2500)));
}
#[test]
fn tts_voice_setting_defaults_and_validates() {
    let db = test_db();
    assert_eq!(tts_voice_setting(&db), "nova");
    db.set_setting("tts_voice", "shimmer").unwrap();
    assert_eq!(tts_voice_setting(&db), "shimmer");
    db.set_setting("tts_voice", "bogus").unwrap();
    assert_eq!(tts_voice_setting(&db), "nova"); // invalid → default
}
```

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test --workspace && cargo check --workspace`. (The end-to-end voice reply needs live creds — assert the pure suitability/voice-setting logic; note the manual WhatsApp check in the report.)
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Reply to WhatsApp voice notes with a synthesized voice note, text fallback

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 4: Server /voice endpoints

**Files:**
- Create: `donna-server/src/voice.rs`
- Modify: `donna-server/src/lib.rs` (routes + mod)
- Test: `donna-server/tests/voice.rs`

**Interfaces:**
- `voice.rs`:
  - `transcribe(State, Multipart) -> Response` — POST `/voice/transcribe`, `multipart/form-data` with a `file` part (audio bytes + filename). Read the part bytes + filename (axum `Multipart`); fetch `secrets::get_api_key("openai")` → None → 400 `{"error":"OpenAI key required for voice"}`; else `audio::transcribe(&key, DEFAULT_TRANSCRIBE_MODEL, bytes, &filename)` → 200 `{"transcript": text}`; transcribe error → 502 `{"error": ...}`.
  - `speak(State, Json<SpeakReq>) -> Response` — POST `/voice/speak`, body `{text, voice?}` → validate voice (else default), `audio::synthesize(&key, DEFAULT_TTS_MODEL, voice, &text, "mp3")` → 200 with `Content-Type: audio/mpeg` and the bytes as the body; no key → 400; synth error → 502.
- lib.rs: register BOTH routes BEFORE `.layer(require_bearer)` (so they inherit bearer auth like /rpc and /ws — NOT after, which is the unauthenticated zone). Add `axum` multipart: axum 0.7 has `Multipart` under the `multipart` feature — check donna-server Cargo.toml, add `features = ["multipart"]` to axum if missing.
- `pub mod voice;`.

- [ ] **Step 1: Failing tests** (`donna-server/tests/voice.rs`, oneshot style):

```rust
// Both endpoints require bearer (they're inside the auth layer) and require an OpenAI key.
// test_state() has no OpenAI key configured → both return 400 (key required), which still proves:
//   (a) the routes exist and are wired, (b) they're behind bearer (no-bearer → 401), (c) the no-key path is handled.
#[tokio::test]
async fn voice_routes_need_bearer_then_report_missing_key() {
    let app = donna_server::build_app(donna_server::test_state());
    // no bearer → 401
    let res = app.clone().oneshot(Request::post("/voice/speak")
        .header("content-type","application/json").body(Body::from(r#"{"text":"hi"}"#)).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    // with bearer, no OpenAI key in test_state → 400 key-required
    let res = app.oneshot(Request::post("/voice/speak")
        .header("authorization","Bearer test-token")
        .header("content-type","application/json").body(Body::from(r#"{"text":"hi"}"#)).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
```
(A real transcribe/synthesize round-trip needs an OpenAI key + network — out of scope for unit tests; the no-key + auth wiring is what's deterministically testable. Note the manual check in the report.)

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test --workspace && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add /voice/transcribe and /voice/speak server endpoints

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 5: Desktop voice UI + mic entitlement + settings + docs

**Files:**
- Create: `src/lib/voice.ts` (record + transcribe + speak helpers)
- Modify: `src/routes/Chat.tsx` (mic button + speak-replies), `src/routes/Settings.tsx` (voice controls), `src/lib/api.ts`/`server.ts` (voice fetch helpers), `src-tauri/Info.plist` (create — mic entitlement) or `src-tauri/tauri.conf.json` (bundle macOS), `donna-server/README.md`, `docs/ROADMAP.md`
- Verify: tsc/build + cargo check + full suite

**Interfaces:**
- `voice.ts`:
  - `recordAudio(): Promise<{ stop: () => Promise<Blob> }>` — `getUserMedia({audio:true})` + `MediaRecorder` (mimeType `audio/webm` if supported); `stop()` resolves the recorded Blob and stops tracks.
  - `transcribe(blob: Blob): Promise<string>` — `FormData` with `file` (blob, filename `clip.webm`) → `fetch(${serverUrl}/voice/transcribe, { method:POST, headers:{Authorization: Bearer <token>}, body: form })` → `{transcript}`.
  - `speak(text: string): Promise<void>` — POST `/voice/speak` `{text}` with bearer → `res.blob()` → `new Audio(URL.createObjectURL(blob)).play()`. (serverConfig() gives url+token, like server.ts rpc.)
- Chat.tsx: a mic button in the composer. Press → `recordAudio()` (show a recording indicator); press again → stop → `transcribe(blob)` → put the transcript in the input and immediately run the existing `sendMessage()` flow (so the reply streams over the normal WS path). A "speak replies" toggle (state persisted to a setting via config, see Settings) — when on, after a streamed reply completes (the existing done handling), call `voice.speak(finalAssistantText)`. Handle mic-permission-denied gracefully (toast: "Microphone access denied — enable it in System Settings").
- Settings.tsx: a "Voice" card — "Speak replies aloud" toggle (rides the config blob: add `speakReplies: boolean` to AppConfig + a `speak_replies` setting, mirroring how `review_model` was wired in Phase 4) and a voice picker (`tts_voice` setting; the same nova/shimmer/... list; immediate-save like the server card OR via config Save — match review_model's pattern). Persist `tts_voice` to the exact setting `whatsapp_handle_audio`/`tts_voice_setting` reads.
- Mic entitlement: create `src-tauri/Info.plist` with `NSMicrophoneUsageDescription` = "Donna needs the microphone so you can talk to her." (Tauri 2 merges `src-tauri/Info.plist` into the bundle.) If tauri.conf.json is the project's convention instead, add it there — READ tauri.conf.json first and follow whatever exists; the goal is the usage-description string reaches the built Info.plist.
- README + ROADMAP: a "Voice" section (mic permission on first use; requires an OpenAI key; WhatsApp voice notes work automatically once WhatsApp is set up) + check Phase 5 items.

- [ ] **Step 1: Implement voice.ts + Chat.tsx mic/playback + Settings voice card + api/config wiring + Info.plist + docs.** **Step 2:** `npx tsc --noEmit && npm run build && cargo test --workspace && cargo check --workspace` clean/green. **Step 3:** report a manual smoke checklist (grant mic on first record; speak a message → transcript appears → reply streams; toggle speak-replies → hear the reply; the desktop mic + TTS playback are the owner-verified pieces).
- [ ] **Step 4: Commit and push**

```bash
git add -A
git commit -m "Desktop voice: mic capture, transcription-to-chat, spoken replies, settings, docs

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

## Done criteria (whole phase)

1. `cargo test --workspace` green; tsc/build clean.
2. `audio::transcribe`/`synthesize` build (multipart feature) and are covered by pure-builder tests; voice validation + mime guessing tested.
3. A WhatsApp voice note (webhook `type:"audio"`) downloads, transcribes, runs the brain, and replies with a voice note (text fallback) — code path complete; owner verifies live.
4. `/voice/transcribe` and `/voice/speak` exist behind bearer auth, return 400 without an OpenAI key, and are wired into the router.
5. The desktop chat has a working mic button (record → transcribe → normal streamed reply) and a "speak replies" toggle that plays her voice; the macOS mic entitlement is present.
6. Voice everywhere fails gracefully without an OpenAI key (never crashes, clear message).

## Follow-ups noted during planning (not in scope)

- Phone-call voice (Twilio ConversationRelay) — deferred per spec.
- Streaming/real-time transcription (vs record-then-send) — record-then-send is simpler and fine for push-to-talk.
- Wake word / always-listening — out of scope; push-to-talk only.
- Barge-in / interrupting playback — click-to-stop is enough for v1.

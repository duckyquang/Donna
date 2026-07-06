//! Quick-chat overlay: screen capture + context for the Cmd+D floating panel.

use base64::Engine;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuickChatContext {
    /// PNG screenshot encoded as base64, or None if unavailable.
    pub screenshot_b64: Option<String>,
    /// Name of the frontmost application when Cmd+D was pressed.
    pub app_name: String,
}

/// App-state wrapper so commands can read the last-captured context.
pub struct QuickChatState {
    pub ctx: Mutex<QuickChatContext>,
}

impl Default for QuickChatState {
    fn default() -> Self {
        Self { ctx: Mutex::new(QuickChatContext::default()) }
    }
}

/// Capture a screenshot and the active app name, return a new context.
pub fn capture_context() -> QuickChatContext {
    QuickChatContext {
        screenshot_b64: capture_screenshot_base64().ok(),
        app_name: frontmost_app(),
    }
}

fn capture_screenshot_base64() -> Result<String> {
    let tmp = std::env::temp_dir().join("donna_quickchat.png");
    let tmp_str = tmp.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    std::process::Command::new("screencapture")
        .args(["-x", "-t", "png", &tmp_str])
        .status()
        .map_err(|e| crate::error::Error::Provider(e.to_string()))?;

    #[cfg(not(target_os = "macos"))]
    return Err(crate::error::Error::Provider("Screenshot not supported on this platform".into()));

    let bytes = std::fs::read(&tmp)
        .map_err(|e| crate::error::Error::Provider(e.to_string()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

fn frontmost_app() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = std::process::Command::new("osascript")
            .args(["-e", "tell application \"System Events\" to get name of first process whose frontmost is true"])
            .output()
        {
            let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !name.is_empty() { return name; }
        }
    }
    "your current task".to_string()
}

/// Create or show the quick-chat window. Call this from the shortcut handler.
pub fn open_quick_chat_window<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("quick-chat") {
        // Window already exists — show, focus, and tell React to refresh context.
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("quick-chat-refresh", ());
    } else {
        tauri::WebviewWindowBuilder::new(
            app,
            "quick-chat",
            tauri::WebviewUrl::App("quick-chat".into()),
        )
        .title("Donna")
        .inner_size(520.0, 500.0)
        .always_on_top(true)
        .decorations(false)
        .center()
        .skip_taskbar(true)
        .build()?;
    }
    Ok(())
}

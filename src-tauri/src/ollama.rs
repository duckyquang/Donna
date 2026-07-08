//! Managed Ollama runtime: Donna downloads the portable runtime once into her app-data
//! dir, runs `ollama serve` herself, and streams model pulls — no terminal, no admin
//! rights. If the user already runs their own Ollama on 11434, we use it untouched.

use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Pinned Ollama release. Bump deliberately; asset names are release-specific.
pub const OLLAMA_VERSION: &str = "v0.31.1"; // ← value from Task 5 Step 1
const OLLAMA_URL: &str = "http://127.0.0.1:11434";

#[derive(Default)]
pub struct OllamaState(pub Mutex<Option<std::process::Child>>);

#[derive(Clone, Serialize)]
pub struct OllamaProgress {
    pub phase: String,
    pub detail: String,
    pub completed: u64,
    pub total: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaInfo {
    pub running: bool,
    pub managed_installed: bool,
    pub models: Vec<String>,
}

/// Release asset for this OS/arch (values from `std::env::consts`).
// Deviation from Task 5 brief: as of OLLAMA_VERSION the upstream release renamed the
// Linux assets from `ollama-linux-{amd64,arm64}.tgz` to `ollama-linux-{amd64,arm64}.tar.zst`
// (zstd instead of gzip). macOS/Windows names are unchanged. `tar -xf` (bsdtar) reads
// zstd-compressed archives transparently, so the extraction code path below is unchanged.
pub fn asset_name(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", _) => Some("ollama-darwin.tgz"),
        ("windows", "x86_64") => Some("ollama-windows-amd64.zip"),
        ("linux", "x86_64") => Some("ollama-linux-amd64.tar.zst"),
        ("linux", "aarch64") => Some("ollama-linux-arm64.tar.zst"),
        _ => None,
    }
}

fn runtime_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app data dir").join("ollama")
}

fn exe_path(dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        dir.join("ollama.exe")
    } else if cfg!(target_os = "linux") {
        dir.join("bin").join("ollama") // linux tgz layout: bin/ollama + lib/ollama
    } else {
        dir.join("ollama")
    }
}

async fn list_models() -> Option<Vec<String>> {
    let v: serde_json::Value = reqwest::Client::new()
        .get(format!("{OLLAMA_URL}/api/tags"))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    Some(
        v["models"]
            .as_array()?
            .iter()
            .filter_map(|m| m["name"].as_str().map(String::from))
            .collect(),
    )
}

#[tauri::command]
pub async fn ollama_status(app: tauri::AppHandle) -> OllamaInfo {
    let models = list_models().await;
    OllamaInfo {
        running: models.is_some(),
        managed_installed: exe_path(&runtime_dir(&app)).exists(),
        models: models.unwrap_or_default(),
    }
}

#[tauri::command]
pub async fn ollama_install(app: tauri::AppHandle) -> Result<(), String> {
    let asset = asset_name(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or("unsupported platform for the built-in local model runtime")?;
    let dir = runtime_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let url =
        format!("https://github.com/ollama/ollama/releases/download/{OLLAMA_VERSION}/{asset}");
    let res = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("runtime download failed: HTTP {}", res.status()));
    }
    let total = res.content_length().unwrap_or(0);
    let archive = dir.join(asset);
    let mut file = std::fs::File::create(&archive).map_err(|e| e.to_string())?;
    let mut stream = res.bytes_stream();
    let mut done: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        let _ = app.emit(
            "ollama:progress",
            OllamaProgress {
                phase: "download".into(),
                detail: asset.into(),
                completed: done,
                total,
            },
        );
    }
    drop(file);
    // One extraction path everywhere: macOS/Linux tar reads .tgz natively, and
    // Windows ships bsdtar (zip-capable) in System32 since Windows 10.
    let out = std::process::Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "runtime extract failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let _ = std::fs::remove_file(&archive);
    if !exe_path(&dir).exists() {
        return Err("runtime extracted but the ollama binary was not found".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn ollama_start(app: tauri::AppHandle) -> Result<(), String> {
    if list_models().await.is_some() {
        return Ok(()); // an Ollama (user's own or ours) is already serving
    }
    let exe = exe_path(&runtime_dir(&app));
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("serve").env("OLLAMA_HOST", "127.0.0.1:11434");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    *app.state::<OllamaState>().0.lock().unwrap() = Some(child);
    for _ in 0..40 {
        if list_models().await.is_some() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    Err("the local model runtime did not start within 10s".into())
}

#[tauri::command]
pub async fn ollama_pull(app: tauri::AppHandle, model: String) -> Result<(), String> {
    let res = reqwest::Client::new()
        .post(format!("{OLLAMA_URL}/api/pull"))
        .json(&serde_json::json!({ "name": model, "stream": true }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("model download failed: HTTP {}", res.status()));
    }
    let mut stream = res.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        while let Some(nl) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=nl).collect();
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&line) {
                if let Some(err) = v["error"].as_str() {
                    return Err(err.to_string());
                }
                let _ = app.emit(
                    "ollama:progress",
                    OllamaProgress {
                        phase: "pull".into(),
                        detail: v["status"].as_str().unwrap_or("").to_string(),
                        completed: v["completed"].as_u64().unwrap_or(0),
                        total: v["total"].as_u64().unwrap_or(0),
                    },
                );
            }
        }
    }
    Ok(())
}

/// Kill the managed runtime on app exit. Never touches a user-installed Ollama.
pub fn kill(app: &tauri::AppHandle) {
    if let Some(mut child) = app.state::<OllamaState>().0.lock().unwrap().take() {
        let _ = child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_cover_supported_platforms() {
        assert_eq!(asset_name("macos", "aarch64"), Some("ollama-darwin.tgz"));
        assert_eq!(asset_name("macos", "x86_64"), Some("ollama-darwin.tgz"));
        assert_eq!(asset_name("windows", "x86_64"), Some("ollama-windows-amd64.zip"));
        assert_eq!(asset_name("linux", "x86_64"), Some("ollama-linux-amd64.tar.zst"));
        assert_eq!(asset_name("linux", "aarch64"), Some("ollama-linux-arm64.tar.zst"));
        assert_eq!(asset_name("freebsd", "x86_64"), None);
    }
}

//! File-based knowledge base.
//!
//! Donna's knowledge lives as a folder tree on disk:
//! - a top-level folder is a **category**,
//! - a nested folder is a **branch**,
//! - a Markdown file is a **node** (with optional image alongside it).
//!
//! This module reads and writes that tree. It is intentionally database-free so the data
//! is transparent, portable, and easy for the user to inspect or back up.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

use crate::error::{Error, Result};

/// Categories seeded on first run so the user starts with a visible structure.
const DEFAULT_CATEGORIES: &[&str] =
    &["About You", "Work", "Study", "Routines", "People", "Projects", "Feedback"];

#[derive(Debug, Serialize, Clone)]
pub struct KbFolder {
    /// Path components relative to the KB root, e.g. ["Routines", "Mornings"].
    pub path: Vec<String>,
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct KbNode {
    pub id: String,
    pub label: String,
    /// Folder (category + branches) the node lives in.
    pub folder: Vec<String>,
    pub note: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub has_image: bool,
    pub updated: String,
}

#[derive(Debug, Serialize)]
pub struct KbGraph {
    pub folders: Vec<KbFolder>,
    pub nodes: Vec<KbNode>,
}

// --- Root resolution --------------------------------------------------------

/// Resolve the knowledge-base directory. Prefers `DONNA_KB_DIR`, then a `knowledge-base`
/// folder at the repo root (the nearest ancestor containing `package.json`), then a
/// `knowledge-base` folder under the current directory.
pub fn kb_root() -> PathBuf {
    if let Ok(p) = std::env::var("DONNA_KB_DIR") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir: Option<&Path> = Some(cwd.as_path());
        while let Some(d) = dir {
            if d.join("package.json").exists() {
                return d.join("knowledge-base");
            }
            dir = d.parent();
        }
    }
    PathBuf::from("knowledge-base")
}

/// Create the KB root and seed default category folders if missing.
pub fn ensure_root() -> Result<()> {
    let root = kb_root();
    std::fs::create_dir_all(&root).map_err(io)?;
    for cat in DEFAULT_CATEGORIES {
        std::fs::create_dir_all(root.join(cat)).map_err(io)?;
    }
    Ok(())
}

fn io(e: std::io::Error) -> Error {
    Error::Provider(format!("knowledge-base IO error: {e}"))
}

/// Resolve a folder path under the root, guarding against path traversal.
fn folder_path(folder: &[String]) -> Result<PathBuf> {
    let mut p = kb_root();
    for part in folder {
        let clean = part.trim();
        if clean.is_empty() || clean.contains('/') || clean.contains('\\') || clean == ".." {
            return Err(Error::Provider("invalid folder segment".into()));
        }
        p.push(clean);
    }
    Ok(p)
}

// --- Slug & frontmatter -----------------------------------------------------

pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in input.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "note".into()
    } else {
        s
    }
}

struct Parsed {
    label: String,
    node_type: String,
    image: Option<String>,
    updated: String,
    note: String,
}

fn parse_node_file(content: &str, fallback_label: &str) -> Parsed {
    let mut label = fallback_label.to_string();
    let mut node_type = "info".to_string();
    let mut image = None;
    let mut updated = String::new();
    let mut note = content.to_string();

    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let front = &rest[..end];
            let body_start = end + "\n---".len();
            note = rest[body_start..].trim_start_matches('\n').to_string();
            for line in front.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let v = v.trim().to_string();
                    match k.trim() {
                        "label" => label = if v.is_empty() { label } else { v },
                        "type" => node_type = if v.is_empty() { node_type } else { v },
                        "image" => image = if v.is_empty() { None } else { Some(v) },
                        "updated" => updated = v,
                        _ => {}
                    }
                }
            }
        }
    }
    Parsed {
        label,
        node_type,
        image,
        updated,
        note: note.trim().to_string(),
    }
}

fn serialize_node_file(label: &str, node_type: &str, image: &Option<String>, note: &str) -> String {
    format!(
        "---\nlabel: {}\ntype: {}\nimage: {}\nupdated: {}\n---\n{}\n",
        label,
        node_type,
        image.clone().unwrap_or_default(),
        now(),
        note
    )
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

// --- Scanning ---------------------------------------------------------------

pub fn graph() -> Result<KbGraph> {
    ensure_root()?;
    let root = kb_root();
    let mut folders = Vec::new();
    let mut nodes = Vec::new();
    scan(&root, &mut Vec::new(), &mut folders, &mut nodes)?;
    Ok(KbGraph { folders, nodes })
}

fn scan(
    dir: &Path,
    rel: &mut Vec<String>,
    folders: &mut Vec<KbFolder>,
    nodes: &mut Vec<KbNode>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            rel.push(name.clone());
            folders.push(KbFolder {
                path: rel.clone(),
                name,
            });
            scan(&path, rel, folders, nodes)?;
            rel.pop();
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if rel.is_empty() {
                // Skip top-level files like README.md; nodes live inside categories.
                continue;
            }
            let id = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let parsed = parse_node_file(&content, &id);
            let has_image = parsed
                .image
                .as_ref()
                .map(|img| dir.join(img).exists())
                .unwrap_or(false);
            nodes.push(KbNode {
                id,
                label: parsed.label,
                folder: rel.clone(),
                note: parsed.note,
                node_type: parsed.node_type,
                has_image,
                updated: parsed.updated,
            });
        }
    }
    Ok(())
}

// --- Mutations --------------------------------------------------------------

/// Create or update a node. If `from_folder`/`from_id` are given and the location or id
/// changes, the old file (and its image) is moved/removed. Returns the saved node.
#[allow(clippy::too_many_arguments)]
pub fn save_node(
    folder: &[String],
    label: &str,
    note: &str,
    node_type: &str,
    from_folder: Option<&[String]>,
    from_id: Option<&str>,
) -> Result<KbNode> {
    if folder.is_empty() {
        return Err(Error::Provider("a node needs a category".into()));
    }
    let dir = folder_path(folder)?;
    std::fs::create_dir_all(&dir).map_err(io)?;
    let id = slugify(label);

    // Carry over an existing image if we are editing in place.
    let mut image: Option<String> = None;
    if let (Some(ff), Some(fid)) = (from_folder, from_id) {
        if let Ok(old_dir) = folder_path(ff) {
            let old_md = old_dir.join(format!("{fid}.md"));
            if old_md.exists() {
                let parsed = parse_node_file(
                    &std::fs::read_to_string(&old_md).unwrap_or_default(),
                    fid,
                );
                if let Some(img) = parsed.image {
                    // Move the image next to the new node file.
                    let src = old_dir.join(&img);
                    if src.exists() {
                        let ext = Path::new(&img)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("png");
                        let new_img = format!("{id}.{ext}");
                        let _ = std::fs::copy(&src, dir.join(&new_img));
                        if src != dir.join(&new_img) {
                            let _ = std::fs::remove_file(&src);
                        }
                        image = Some(new_img);
                    }
                }
                // Remove the old node file if it is not the same target.
                if old_md != dir.join(format!("{id}.md")) {
                    let _ = std::fs::remove_file(&old_md);
                }
            }
        }
    }

    let content = serialize_node_file(label, node_type, &image, note);
    std::fs::write(dir.join(format!("{id}.md")), content).map_err(io)?;

    Ok(KbNode {
        id: id.clone(),
        label: label.to_string(),
        folder: folder.to_vec(),
        note: note.to_string(),
        node_type: node_type.to_string(),
        has_image: image.is_some(),
        updated: now(),
    })
}

pub fn delete_node(folder: &[String], id: &str) -> Result<()> {
    let dir = folder_path(folder)?;
    let md = dir.join(format!("{id}.md"));
    if md.exists() {
        // Remove an associated image too.
        let parsed = parse_node_file(&std::fs::read_to_string(&md).unwrap_or_default(), id);
        if let Some(img) = parsed.image {
            let _ = std::fs::remove_file(dir.join(img));
        }
        std::fs::remove_file(&md).map_err(io)?;
    }
    Ok(())
}

/// Copy an external image into the node's folder and record it in the frontmatter.
pub fn set_node_image(folder: &[String], id: &str, source_path: &str) -> Result<()> {
    let dir = folder_path(folder)?;
    let md = dir.join(format!("{id}.md"));
    if !md.exists() {
        return Err(Error::Provider("node not found".into()));
    }
    let src = Path::new(source_path);
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let img_name = format!("{id}.{ext}");
    std::fs::copy(src, dir.join(&img_name)).map_err(io)?;

    let parsed = parse_node_file(&std::fs::read_to_string(&md).unwrap_or_default(), id);
    let content = serialize_node_file(&parsed.label, &parsed.node_type, &Some(img_name), &parsed.note);
    std::fs::write(&md, content).map_err(io)?;
    Ok(())
}

pub fn remove_node_image(folder: &[String], id: &str) -> Result<()> {
    let dir = folder_path(folder)?;
    let md = dir.join(format!("{id}.md"));
    if !md.exists() {
        return Ok(());
    }
    let parsed = parse_node_file(&std::fs::read_to_string(&md).unwrap_or_default(), id);
    if let Some(img) = &parsed.image {
        let _ = std::fs::remove_file(dir.join(img));
    }
    let content = serialize_node_file(&parsed.label, &parsed.node_type, &None, &parsed.note);
    std::fs::write(&md, content).map_err(io)?;
    Ok(())
}

/// Return the node's image as a `data:` URL, if it has one.
pub fn node_image(folder: &[String], id: &str) -> Result<Option<String>> {
    let dir = folder_path(folder)?;
    let md = dir.join(format!("{id}.md"));
    if !md.exists() {
        return Ok(None);
    }
    let parsed = parse_node_file(&std::fs::read_to_string(&md).unwrap_or_default(), id);
    let img = match parsed.image {
        Some(i) => i,
        None => return Ok(None),
    };
    let img_path = dir.join(&img);
    if !img_path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&img_path).map_err(io)?;
    let mime = match Path::new(&img)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(Some(format!("data:{mime};base64,{b64}")))
}

/// Delete everything in the knowledge base (except the README) and re-seed the default
/// categories. Used by the Mind Map "Reset knowledge" action.
pub fn reset() -> Result<()> {
    let root = kb_root();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else if path.file_name().and_then(|n| n.to_str()) != Some("README.md") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    ensure_root()
}

/// Compact summary of everything Donna knows, injected into the chat system prompt so
/// she can contrast known vs unknown facts before asking questions.
pub fn summary_for_prompt() -> Result<String> {
    let g = graph()?;
    if g.nodes.is_empty() {
        return Ok("(empty — Donna does not know anything about this user yet)".into());
    }
    let mut lines: Vec<String> = g
        .nodes
        .iter()
        .map(|n| {
            let folder = n.folder.join(" / ");
            let note = if n.note.len() > 100 {
                format!("{}…", &n.note[..100])
            } else {
                n.note.clone()
            };
            format!("- [{folder}] {} ({}) — {note}", n.label, n.node_type)
        })
        .collect();
    lines.sort();
    Ok(lines.join("\n"))
}

/// Existing category names (top-level folders), for prompting and the UI.
pub fn categories() -> Result<Vec<String>> {
    let g = graph()?;
    let mut cats: Vec<String> = g
        .folders
        .iter()
        .filter(|f| f.path.len() == 1)
        .map(|f| f.name.clone())
        .collect();
    cats.sort();
    cats.dedup();
    Ok(cats)
}

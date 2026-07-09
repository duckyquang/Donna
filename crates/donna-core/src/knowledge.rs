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

/// Indented folder tree with node labels — helps the curator reuse branches.
pub fn tree_context_for_prompt() -> Result<String> {
    let g = graph()?;
    if g.folders.is_empty() && g.nodes.is_empty() {
        return Ok("(empty — no folders or nodes yet)".into());
    }

    let mut sorted = g.folders.clone();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut lines = Vec::new();
    for folder in &sorted {
        let depth = folder.path.len();
        let indent = "  ".repeat(depth.saturating_sub(1));
        let mut line = format!("{indent}{}/", folder.name);
        let nodes_here: Vec<&str> = g
            .nodes
            .iter()
            .filter(|n| n.folder == folder.path)
            .map(|n| n.label.as_str())
            .collect();
        if !nodes_here.is_empty() {
            line.push_str(&format!(" — {}", nodes_here.join(", ")));
        }
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

/// Globally-unique id for a folder branch in the mind map.
pub fn folder_node_id(path: &[String]) -> String {
    format!("folder:{}", path.join("/"))
}

/// Globally-unique id for a content node file in the mind map.
pub fn content_node_id(folder: &[String], file_id: &str) -> String {
    format!("{}/{}", folder.join("/"), file_id)
}

/// Parent → child edges that mirror the on-disk folder hierarchy.
pub fn hierarchy_edges(g: &KbGraph) -> Vec<(String, String)> {
    use std::collections::HashSet;

    let mut edges = Vec::new();
    let mut seen = HashSet::new();

    let mut add = |source: &str, target: &str| {
        if source != target && seen.insert((source.to_string(), target.to_string())) {
            edges.push((source.to_string(), target.to_string()));
        }
    };

    for folder in &g.folders {
        let child = folder_node_id(&folder.path);
        if folder.path.len() > 1 {
            let parent = folder_node_id(&folder.path[..folder.path.len() - 1]);
            add(&parent, &child);
        }
    }

    for node in &g.nodes {
        let content = content_node_id(&node.folder, &node.id);
        let container = folder_node_id(&node.folder);
        add(&container, &content);
    }

    edges
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

struct BasicField {
    label: &'static str,
    prompt_hint: &'static str,
    check: fn(&[KbNode]) -> bool,
}

fn haystack(node: &KbNode) -> String {
    format!("{} {}", node.label, node.note).to_lowercase()
}

fn note_is_substantive(note: &str) -> bool {
    let trimmed = note.trim();
    trimmed.len() >= 2
        && !trimmed
            .to_lowercase()
            .starts_with("user prefers to be addressed")
}

fn knows_preferred_name(nodes: &[KbNode]) -> bool {
    nodes.iter().any(|n| {
        let label = n.label.to_lowercase();
        if !(label.contains("name") || label.contains("nickname") || label.contains("call me")) {
            return false;
        }
        let note = n.note.trim();
        if !note_is_substantive(note) {
            return false;
        }
        let lower = note.to_lowercase();
        ![
            "prefers to be addressed",
            "user prefers",
            "prefer to be called",
            "by a nickname",
            "not specified",
            "unknown",
        ]
        .iter()
        .any(|v| lower.contains(v))
    })
}

fn knows_age(nodes: &[KbNode]) -> bool {
    nodes.iter().any(|n| {
        let hay = haystack(n);
        (hay.contains("age") || hay.contains("years old") || hay.contains("year-old"))
            && hay.chars().any(|c| c.is_ascii_digit())
    })
}

fn knows_birthday(nodes: &[KbNode]) -> bool {
    nodes.iter().any(|n| {
        let hay = haystack(n);
        hay.contains("birthday")
            || hay.contains("birth date")
            || hay.contains("date of birth")
            || hay.contains("born on")
            || hay.contains("dob")
    })
}

fn knows_nationality(nodes: &[KbNode]) -> bool {
    nodes.iter().any(|n| {
        let hay = haystack(n);
        hay.contains("nationality")
            || hay.contains("national")
            || hay.contains("citizen")
            || hay.contains("country of origin")
            || (hay.contains("from ") && hay.len() > 12)
    })
}

fn knows_location(nodes: &[KbNode]) -> bool {
    nodes.iter().any(|n| {
        let hay = haystack(n);
        hay.contains("timezone")
            || hay.contains("time zone")
            || hay.contains("city")
            || hay.contains("located")
            || hay.contains("lives in")
            || hay.contains("based in")
    })
}

fn knows_work_or_study(nodes: &[KbNode]) -> bool {
    nodes.iter().any(|n| {
        n.folder
            .first()
            .is_some_and(|f| f == "Work" || f == "Study")
            || {
                let hay = haystack(n);
                hay.contains("employer")
                    || hay.contains("works at")
                    || hay.contains("studies at")
                    || hay.contains("student at")
                    || hay.contains("job title")
                    || hay.contains("current role")
            }
    })
}

const BASIC_FIELDS: &[BasicField] = &[
    BasicField {
        label: "Preferred name",
        prompt_hint: "what should Donna call you?",
        check: knows_preferred_name,
    },
    BasicField {
        label: "Age",
        prompt_hint: "how old are you, or what age range?",
        check: knows_age,
    },
    BasicField {
        label: "Nationality",
        prompt_hint: "what nationality or country do you identify with?",
        check: knows_nationality,
    },
    BasicField {
        label: "Birthday",
        prompt_hint: "when is your birthday?",
        check: knows_birthday,
    },
    BasicField {
        label: "Location / timezone",
        prompt_hint: "what city or timezone are you in?",
        check: knows_location,
    },
    BasicField {
        label: "Work or study",
        prompt_hint: "what do you do for work or study?",
        check: knows_work_or_study,
    },
];

const BASIC_FIELD_IDS: &[&str] = &[
    "preferred_name",
    "age",
    "nationality",
    "birthday",
    "location",
    "work_or_study",
];

#[derive(Debug, Serialize)]
pub struct BasicFieldStatus {
    pub id: String,
    pub label: String,
    pub prompt_hint: String,
    pub known: bool,
}

/// Structured basics status for the first-conversation profile onboarding UI.
pub fn basics_status() -> Result<Vec<BasicFieldStatus>> {
    let nodes = graph()?.nodes;
    Ok(BASIC_FIELDS
        .iter()
        .enumerate()
        .map(|(i, field)| BasicFieldStatus {
            id: BASIC_FIELD_IDS[i].into(),
            label: field.label.into(),
            prompt_hint: field.prompt_hint.into(),
            known: (field.check)(&nodes),
        })
        .collect())
}

// --- Capped memory files (USER.md / MEMORY.md) ------------------------------

/// Char cap for USER.md (stable identity/preferences).
pub const USER_MD_CAP: usize = 1500;
/// Char cap for MEMORY.md (active threads/conventions).
pub const MEMORY_MD_CAP: usize = 2500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryFile {
    User,
    Memory,
}

impl MemoryFile {
    fn filename(self) -> &'static str {
        match self {
            MemoryFile::User => "USER.md",
            MemoryFile::Memory => "MEMORY.md",
        }
    }

    fn cap(self) -> usize {
        match self {
            MemoryFile::User => USER_MD_CAP,
            MemoryFile::Memory => MEMORY_MD_CAP,
        }
    }

    fn heading(self) -> &'static str {
        match self {
            MemoryFile::User => "## About you",
            MemoryFile::Memory => "## Working memory",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAction {
    Add,
    Replace,
    Remove,
}

fn cap_check(body: &str, cap: usize) -> bool {
    body.chars().count() <= cap
}

/// Read a memory file's contents, or "" if it does not exist yet.
pub fn read_memory_file(which: MemoryFile) -> Result<String> {
    let path = kb_root().join(which.filename());
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(io(e)),
    }
}

fn write_memory_file(which: MemoryFile, body: &str) -> Result<()> {
    std::fs::create_dir_all(kb_root()).map_err(io)?;
    std::fs::write(kb_root().join(which.filename()), body).map_err(io)
}

/// The `## About you` / `## Working memory` block injected into the system prompt.
/// A file's block is omitted when that file is empty; "" when both are empty.
pub fn memory_prompt_section() -> Result<String> {
    let mut blocks = Vec::new();
    for which in [MemoryFile::User, MemoryFile::Memory] {
        let body = read_memory_file(which)?;
        if !body.trim().is_empty() {
            blocks.push(format!("{}\n{}", which.heading(), body.trim()));
        }
    }
    Ok(blocks.join("\n\n"))
}

/// Apply an update to a memory file. `Add` appends `text` as a new line (errors
/// `MEMORY_FULL: …` with the current contents if the result would exceed the cap so the
/// model can consolidate instead). `Replace` rewrites the whole body to `text` (errors if
/// over cap). `Remove` deletes any line containing `text` as a substring. Returns the new
/// contents.
pub fn apply_memory_update(which: MemoryFile, action: MemoryAction, text: &str) -> Result<String> {
    let current = read_memory_file(which)?;
    let cap = which.cap();

    let new_body = match action {
        MemoryAction::Add => {
            let candidate = if current.trim().is_empty() {
                text.to_string()
            } else {
                format!("{}\n{}", current.trim_end_matches('\n'), text)
            };
            if !cap_check(&candidate, cap) {
                return Err(Error::Provider(format!(
                    "MEMORY_FULL: {} is at cap ({cap} chars). Consolidate before adding more.\n\
                    Current contents:\n{current}",
                    which.filename()
                )));
            }
            candidate
        }
        MemoryAction::Replace => {
            if !cap_check(text, cap) {
                return Err(Error::Provider(format!(
                    "MEMORY_FULL: replacement body for {} exceeds cap ({cap} chars).",
                    which.filename()
                )));
            }
            text.to_string()
        }
        MemoryAction::Remove => current
            .lines()
            .filter(|line| !line.contains(text))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    write_memory_file(which, &new_body)?;
    Ok(new_body)
}

/// Human-readable checklist of core identity facts Donna has vs still needs to ask about.
pub fn basics_checklist_for_prompt() -> Result<String> {
    let nodes = graph()?.nodes;
    let mut known = Vec::new();
    let mut missing = Vec::new();

    for field in BASIC_FIELDS {
        if (field.check)(&nodes) {
            known.push(format!("- ✓ {}", field.label));
        } else {
            missing.push(format!(
                "- ☐ {} ({})",
                field.label, field.prompt_hint
            ));
        }
    }

    if missing.is_empty() {
        return Ok("All core identity basics are recorded.".into());
    }

    let mut out = String::from(
        "Donna MUST ask about missing basics (highest priority first) before casual topics:\n",
    );
    out.push_str(&missing.join("\n"));
    if !known.is_empty() {
        out.push_str("\n\nAlready recorded:\n");
        out.push_str(&known.join("\n"));
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // ponytail: DONNA_KB_DIR is a process-wide env var; the mutex keeps tests that set it
    // (in this file and in tools.rs) from racing each other across threads.
    pub(crate) fn kb_env_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    /// Holds the process-wide env lock (released on drop) plus the temp dir path.
    #[must_use]
    #[allow(dead_code)] // .1 (dir path) kept for debugging; guard (.0) is the load-bearing field
    pub(crate) struct TempKb(std::sync::MutexGuard<'static, ()>, PathBuf);

    /// Point DONNA_KB_DIR at a fresh temp dir and seed it. Holds a process-wide lock for
    /// the returned guard's lifetime so concurrent tests (crate-wide) don't race on the env
    /// var. Every test in this crate that sets DONNA_KB_DIR must go through this helper.
    pub(crate) fn temp_kb() -> TempKb {
        let guard = kb_env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "donna-kb-{}-{}",
            std::process::id(),
            crate::db::unique_test_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("DONNA_KB_DIR", &dir);
        ensure_root().unwrap();
        TempKb(guard, dir)
    }

    #[test]
    fn memory_add_until_full_then_errors() {
        let _root = temp_kb();
        assert_eq!(read_memory_file(MemoryFile::User).unwrap(), "");
        apply_memory_update(MemoryFile::User, MemoryAction::Add, "Name: Buno").unwrap();
        assert!(read_memory_file(MemoryFile::User).unwrap().contains("Name: Buno"));
        // fill past the 1500 cap
        let big = "x".repeat(1600);
        let err = apply_memory_update(MemoryFile::User, MemoryAction::Add, &big).unwrap_err();
        assert!(err.to_string().contains("MEMORY_FULL"));
        // replace to shrink works even near cap
        apply_memory_update(MemoryFile::User, MemoryAction::Replace, "Name: B").unwrap();
        assert_eq!(read_memory_file(MemoryFile::User).unwrap().trim(), "Name: B");
        // remove
        apply_memory_update(MemoryFile::User, MemoryAction::Remove, "Name: B").unwrap();
        assert_eq!(read_memory_file(MemoryFile::User).unwrap().trim(), "");
    }

    #[test]
    fn memory_prompt_section_shape() {
        let _root = temp_kb();
        assert_eq!(memory_prompt_section().unwrap(), "");
        apply_memory_update(MemoryFile::User, MemoryAction::Add, "Prefers concise replies").unwrap();
        let s = memory_prompt_section().unwrap();
        assert!(s.contains("## About you"));
        assert!(s.contains("Prefers concise replies"));
        assert!(!s.contains("## Working memory")); // MEMORY.md still empty → omitted
    }
}

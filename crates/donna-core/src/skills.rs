//! File-based skills catalog.
//!
//! A skill is a folder on disk containing a `SKILL.md` file (frontmatter + body) and,
//! optionally, reference files alongside it. This mirrors `knowledge.rs`'s root
//! resolution / traversal-guard / frontmatter approach so the two on-disk stores behave
//! the same way for the user.

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::knowledge::slugify;

// --- Root resolution --------------------------------------------------------

/// Resolve the skills directory. Prefers `DONNA_SKILLS_DIR`, then a `skills` folder at the
/// repo root (the nearest ancestor containing `package.json`), then a `skills` folder under
/// the current directory.
pub fn skills_root() -> PathBuf {
    if let Ok(p) = std::env::var("DONNA_SKILLS_DIR") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir: Option<&std::path::Path> = Some(cwd.as_path());
        while let Some(d) = dir {
            if d.join("package.json").exists() {
                return d.join("skills");
            }
            dir = d.parent();
        }
    }
    PathBuf::from("skills")
}

fn io(e: std::io::Error) -> Error {
    Error::Provider(format!("skills IO error: {e}"))
}

const SEED_SLUG: &str = "weekly-review";
const SEED_BODY: &str = "1. Pull the week's calendar events, meetings, and any docs touched.\n\
2. Summarize wins and blockers from the week.\n\
3. Propose next week's focus based on open threads.\n";

/// Create the skills root and seed one example skill if the directory is newly
/// created / empty, so the catalog is never empty.
pub fn ensure_root() -> Result<()> {
    let root = skills_root();
    let was_empty = !root.exists()
        || std::fs::read_dir(&root)
            .map(|mut e| e.next().is_none())
            .unwrap_or(true);
    std::fs::create_dir_all(&root).map_err(io)?;
    if was_empty {
        save_skill(
            "Weekly Review",
            "Run a structured weekly review",
            "productivity",
            SEED_BODY,
        )?;
        let _ = SEED_SLUG; // slug kept as a named constant for clarity/debugging
    }
    Ok(())
}

/// Guard a single path segment against traversal, mirroring knowledge::folder_path.
fn guard_segment(part: &str) -> Result<&str> {
    let clean = part.trim();
    if clean.is_empty() || clean.contains('/') || clean.contains('\\') || clean == ".." {
        return Err(Error::Provider("invalid path segment".into()));
    }
    Ok(clean)
}

/// Resolve `<skills_root>/<slug>`, guarding the slug against traversal.
fn skill_dir(slug: &str) -> Result<PathBuf> {
    let clean = guard_segment(slug)?;
    Ok(skills_root().join(clean))
}

/// Resolve `<skills_root>/<slug>/<relpath>`, guarding every segment of relpath.
fn skill_ref_path(slug: &str, relpath: &str) -> Result<PathBuf> {
    let mut p = skill_dir(slug)?;
    for part in relpath.split('/') {
        p.push(guard_segment(part)?);
    }
    Ok(p)
}

// --- Frontmatter --------------------------------------------------------------

struct ParsedSkill {
    name: String,
    description: String,
    category: String,
}

fn parse_skill_file(content: &str, fallback_name: &str) -> ParsedSkill {
    let mut name = fallback_name.to_string();
    let mut description = String::new();
    let mut category = String::new();

    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let front = &rest[..end];
            for line in front.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let v = v.trim().to_string();
                    match k.trim() {
                        "name" => name = if v.is_empty() { name } else { v },
                        "description" => description = v,
                        "category" => category = v,
                        _ => {}
                    }
                }
            }
        }
    }
    ParsedSkill {
        name,
        description,
        category,
    }
}

fn serialize_skill_file(name: &str, description: &str, category: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\ncategory: {category}\n---\n{body}\n")
}

// --- Public API ---------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillMeta {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub category: String,
}

/// List all skills that have a `SKILL.md`, sorted by name. Directories without a
/// `SKILL.md` are skipped.
pub fn list_skills() -> Result<Vec<SkillMeta>> {
    let root = skills_root();
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let md = path.join("SKILL.md");
        if !md.exists() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();
        let content = std::fs::read_to_string(&md).unwrap_or_default();
        let parsed = parse_skill_file(&content, &slug);
        out.push(SkillMeta {
            name: parsed.name,
            slug,
            description: parsed.description,
            category: parsed.category,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// View a skill's `SKILL.md` (path = None) or a reference file alongside it
/// (path = Some(relpath)). Both the skill name and every path segment are traversal-guarded.
pub fn view_skill(name: &str, path: Option<&str>) -> Result<String> {
    let slug = slugify(name);
    let target = match path {
        None => skill_dir(&slug)?.join("SKILL.md"),
        Some(p) => skill_ref_path(&slug, p)?,
    };
    if !target.exists() {
        return Err(match path {
            None => Error::Provider(format!("skill '{name}' not found")),
            Some(p) => Error::Provider(format!("reference '{p}' not found")),
        });
    }
    std::fs::read_to_string(&target).map_err(io)
}

/// Create or overwrite a skill's `SKILL.md`. Returns the saved metadata.
pub fn save_skill(name: &str, description: &str, category: &str, body: &str) -> Result<SkillMeta> {
    let slug = slugify(name);
    if slug.is_empty() || slug == "note" {
        return Err(Error::Provider("skill needs a usable name".into()));
    }
    let dir = skill_dir(&slug)?;
    std::fs::create_dir_all(&dir).map_err(io)?;
    let content = serialize_skill_file(name, description, category, body);
    std::fs::write(dir.join("SKILL.md"), content).map_err(io)?;
    Ok(SkillMeta {
        name: name.to_string(),
        slug,
        description: description.to_string(),
        category: category.to_string(),
    })
}

/// `## Available skills` section for the chat system prompt — name + description only,
/// never the skill body. Empty catalog → "".
pub fn skills_prompt_section() -> Result<String> {
    let skills = list_skills()?;
    if skills.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::from("## Available skills\n");
    let lines: Vec<String> = skills
        .iter()
        .map(|s| format!("- {} — {}", s.name, s.description))
        .collect();
    out.push_str(&lines.join("\n"));
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // ponytail: DONNA_SKILLS_DIR is process-wide, same hazard as DONNA_KB_DIR in
    // knowledge.rs — a separate lock since the two env vars are independent.
    fn skills_env_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    #[must_use]
    #[allow(dead_code)] // .1 (dir path) kept for debugging; guard (.0) is load-bearing
    pub(crate) struct TempSkills(std::sync::MutexGuard<'static, ()>, PathBuf);

    /// Point DONNA_SKILLS_DIR at a fresh temp dir and seed it. Holds a process-wide lock
    /// for the returned guard's lifetime so concurrent tests don't race on the env var.
    pub(crate) fn skills_test_guard() -> TempSkills {
        let guard = skills_env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "donna-skills-{}-{}",
            std::process::id(),
            crate::db::unique_test_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("DONNA_SKILLS_DIR", &dir);
        ensure_root().unwrap();
        TempSkills(guard, dir)
    }

    #[test]
    fn save_list_view_roundtrip() {
        let _g = skills_test_guard();
        let m = save_skill(
            "Weekly Review",
            "Run a structured weekly review",
            "productivity",
            "1. Pull the week...\n2. Summarize",
        )
        .unwrap();
        assert_eq!(m.slug, "weekly-review");
        let list = list_skills().unwrap();
        assert!(list
            .iter()
            .any(|s| s.slug == "weekly-review" && s.description.contains("weekly review")));
        let body = view_skill("Weekly Review", None).unwrap();
        assert!(body.contains("Summarize"));
        assert!(view_skill("no-such-skill", None).is_err());
    }

    #[test]
    fn view_rejects_traversal() {
        let _g = skills_test_guard();
        save_skill("A", "a", "x", "body").unwrap();
        assert!(view_skill("A", Some("../../etc/passwd")).is_err());
        assert!(view_skill("..", None).is_err());
    }

    #[test]
    fn prompt_section_lists_name_and_description_only() {
        let _g = skills_test_guard();
        save_skill(
            "Weekly Review",
            "Run a structured weekly review",
            "productivity",
            "SECRET BODY DETAIL",
        )
        .unwrap();
        let s = skills_prompt_section().unwrap();
        assert!(s.contains("## Available skills"));
        assert!(s.contains("Weekly Review"));
        assert!(!s.contains("SECRET BODY DETAIL")); // body never in the listing
    }

    #[test]
    fn ensure_root_seeds_once_idempotent() {
        let _g = skills_test_guard();
        // skills_test_guard already called ensure_root once (seeded weekly-review).
        let after_first = list_skills().unwrap();
        assert_eq!(after_first.len(), 1);
        ensure_root().unwrap();
        let after_second = list_skills().unwrap();
        assert_eq!(after_second.len(), 1); // no duplicate seed
    }
}

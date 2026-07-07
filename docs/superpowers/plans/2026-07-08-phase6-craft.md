# Phase 6: Craft — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Donna has skills to find skills. `SKILL.md` files she discovers through progressive disclosure (sees the catalog, loads one on demand), uses to guide multi-step work, and authors herself — manually via a tool, or proposed by the nightly review when it spots a recurring recipe (Hermes "skill from experience").

**Architecture:** Skills are an on-disk folder of `SKILL.md` files (one dir per skill), mirroring the knowledge-base's root-resolution + traversal guard + YAML-frontmatter read/write. Three new agent tools — `skills_list` (the catalog, name+description only), `skill_view` (load a skill's full SKILL.md or a reference file), `skill_create` (author one). A `## Available skills` Level-0 listing is injected into every system prompt so the model always knows what exists and loads details on demand. Skill authoring rides the Phase-4 suggestion queue: a new `kind: "skill"` accept-arm and a review-prompt rule let Donna propose a skill, consent-first.

**Tech Stack:** existing only — Rust (fs + serde, reusing knowledge.rs patterns), React/TS. No new dependencies.

## Global Constraints

- Spec §4 (skills) + the Hermes progressive-disclosure pattern. This is the LAST spec phase.
- **Commit AND push after every task.** Branch `feat/phase-5-projects-discord-proactive`, no PRs.
- Skills live at `skills_root()`: env `DONNA_SKILLS_DIR` if set/non-empty, else the kb-style fallback with the folder name `skills` (mirror `knowledge::kb_root()` exactly, swapping the env var + folder literal). The server sets `DONNA_SKILLS_DIR` to `<data_dir>/skills` in main.rs (like it sets `DONNA_KB_DIR`).
- Layout: `skills/<slug>/SKILL.md`. `<slug>` = slugified skill name (reuse `knowledge::slugify` — expose it `pub(crate)` if private). SKILL.md frontmatter: `name`, `description`, `category`; body = the instructions. Reference files: `skills/<slug>/<relpath>` (traversal-guarded) for `skill_view(name, path)`.
- Traversal guard: mirror `knowledge::folder_path`'s per-segment check (reject empty, `/`, `\`, `..`) for BOTH the slug and any `path` segments. A skill name/path the model supplies must never escape `skills_root()`.
- Progressive disclosure: `skills_list` and the prompt listing return name+description ONLY (cheap). `skill_view` returns the full body. Never dump full skill bodies into the system prompt.
- `skill_create`/`skills_list`/`skill_view` are agent tools → they run ONLY on the OpenAI agent path (send_chat routes provider=="openai" to the agent loop). The `## Available skills` listing is injected for ALL providers (harmless capability context); the agent addendum (OpenAI-only) is what tells the model to `skill_view` before acting. Non-OpenAI degrades to "sees the catalog, can't load details" — acceptable, consistent with non-OpenAI having no tools at all.
- Suggestion authoring keeps the Phase-4 invariant: parse the skill payload BEFORE `resolve_suggestion` so a malformed payload leaves the suggestion pending/retryable. Dedup key `skill:<slug>` (a dismissed skill suggestion never re-nags).
- Tool count: 33 → 36. Update `TOOL_COUNT` + the two inline `assert_eq!(all().len(), 33)` literals in tools.rs tests.

---

### Task 1: skills module — persistence, list, view

**Files:**
- Create: `crates/donna-core/src/skills.rs` (+ `pub mod skills;` in lib.rs)
- Modify: `crates/donna-core/src/knowledge.rs` (expose `slugify` as `pub(crate)` if private)
- Test: inline in skills.rs

**Interfaces:**
- `pub fn skills_root() -> PathBuf` — mirror knowledge::kb_root (env `DONNA_SKILLS_DIR`, else ancestor-walk `.../skills`, else `./skills`).
- `pub fn ensure_root() -> Result<()>` — mkdir -p skills_root; seed ONE example skill on first run (see below) so the catalog is never empty.
- `fn skill_dir(slug: &str) -> Result<PathBuf>` — skills_root().join(guarded slug); reject bad segments like folder_path.
- `pub struct SkillMeta { pub name: String, pub slug: String, pub description: String, pub category: String }`
- `pub fn list_skills() -> Result<Vec<SkillMeta>>` — read_dir skills_root(); for each subdir with a SKILL.md, parse frontmatter → SkillMeta (name/description/category from frontmatter, slug from dir name); sorted by name; skip dirs without SKILL.md.
- `pub fn view_skill(name: &str, path: Option<&str>) -> Result<String>` — slug = slugify(name); if `path` is None → read `<slug>/SKILL.md`; else read `<slug>/<guarded path>` (guard EACH path segment). Missing → `Err(Error::Provider("skill '<name>' not found"))` / "reference '<path>' not found".
- `pub fn save_skill(name: &str, description: &str, category: &str, body: &str) -> Result<SkillMeta>` — slug = slugify(name) (error if empty); mkdir `<slug>/`; write `SKILL.md` = frontmatter (`---\nname: {name}\ndescription: {description}\ncategory: {category}\n---\n{body}\n`); return the SkillMeta. Overwrites an existing same-slug SKILL.md (edit-in-place).
- `pub fn skills_prompt_section() -> Result<String>` — `## Available skills\n<one line per skill: "- {name} — {description}">` (empty catalog → ""). Used by Task 3.
- Reuse knowledge.rs's frontmatter parse shape (split on first `---`, `\n---`, `k: v` lines); a small local parser is fine (skills only need name/description/category, no image/updated).
- Seed skill (ensure_root, only when the dir is newly created / empty): a "weekly-review" example — SKILL.md with a short body describing how to run a weekly review (pull the week's calendar + meetings + docs, summarize wins/blockers, propose next-week focus). Keeps the pattern visible.

- [ ] **Step 1: Failing tests** (use a temp skills root — set `DONNA_SKILLS_DIR` to a unique temp dir via the shared `unique_test_suffix`, and take the same env-lock guard the KB tests use since `DONNA_SKILLS_DIR` is process-global too — check knowledge.rs's `test_kb_guard`/`kb_env_lock` and add a sibling `skills_env_lock` OR reuse a shared lock):

```rust
#[test]
fn save_list_view_roundtrip() {
    let _g = skills_test_guard(); // sets DONNA_SKILLS_DIR to a fresh temp dir + holds the lock
    let m = save_skill("Weekly Review", "Run a structured weekly review", "productivity", "1. Pull the week...\n2. Summarize").unwrap();
    assert_eq!(m.slug, "weekly-review");
    let list = list_skills().unwrap();
    assert!(list.iter().any(|s| s.slug == "weekly-review" && s.description.contains("weekly review")));
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
    save_skill("Weekly Review", "Run a structured weekly review", "productivity", "SECRET BODY DETAIL").unwrap();
    let s = skills_prompt_section().unwrap();
    assert!(s.contains("## Available skills"));
    assert!(s.contains("Weekly Review"));
    assert!(!s.contains("SECRET BODY DETAIL")); // body never in the listing
}
```

- [ ] **Step 2: RED** — `cargo test -p donna-core skills` → FAIL. **Step 3: Implement** (mirror knowledge.rs root/guard/frontmatter). **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace`, zero new warnings, no flaky env race (run the suite a couple times).
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add skills module: on-disk SKILL.md files with list, view, save, traversal guard

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 2: skill tools (skills_list, skill_view, skill_create)

**Files:**
- Modify: `crates/donna-core/src/tools.rs` (3 ToolDefs + 3 execute arms + count 33→36), `crates/donna-core/src/ops.rs` (thin ops wrappers if the pattern wants them — or call skills:: directly from the execute arms)
- Test: inline in tools.rs

**Interfaces (per the registry pattern — 3 edits each):**
- `skills_list` — Risk::Read, params `{}` (zero-arg like list_docs); execute → `ok(skills::list_skills()?)` (returns the SkillMeta catalog as JSON). Description: "List all of Donna's available skills (name + description only). Call this to see what skills exist, then skill_view to load one's full instructions before acting."
- `skill_view` — Risk::Read, params `{name: string (required), path: string (optional — a reference file inside the skill)}`; execute → `#[derive(Deserialize)] struct A { name: String, #[serde(default)] path: Option<String> }` → `ok(skills::view_skill(&a.name, a.path.as_deref())?)`. Description: "Load a skill's full SKILL.md instructions by name (or a reference file via `path`). Read the skill BEFORE acting on it; follow its steps."
- `skill_create` — Risk::Write, params `{name, description, category, body}` (all required strings); execute → `ok(skills::save_skill(&a.name, &a.description, &a.category, &a.body)?)`. Description: "Author a new reusable skill as a SKILL.md. Use when you've worked out a repeatable multi-step recipe worth saving. name = short title; description = one line for the catalog; category = a grouping; body = the step-by-step instructions in Markdown."
- Bump `TOOL_COUNT` to 36 and the two inline `assert_eq!(all().len(), 33)` → 36.
- No trust.rs change: `risk_of` reads from `all()`, so `trust::decide` maps Read/Write → Auto automatically once the tools are registered.

- [ ] **Step 1: Failing tests**

```rust
#[tokio::test]
async fn skill_tools_registered_and_dispatch() {
    let db = test_db();
    let _g = skills_test_guard(); // from Task 1
    assert_eq!(all().len(), 36);
    let created = execute(&db, "skill_create", &serde_json::json!({
        "name":"Trip Planner","description":"Plan a trip","category":"travel","body":"1. Ask dates\n2. ..."})).await.unwrap();
    assert!(created.contains("trip-planner"));
    let listed = execute(&db, "skills_list", &serde_json::json!({})).await.unwrap();
    assert!(listed.contains("Trip Planner"));
    let viewed = execute(&db, "skill_view", &serde_json::json!({"name":"Trip Planner"})).await.unwrap();
    assert!(viewed.contains("Ask dates"));
}
```

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Add skills_list, skill_view, skill_create agent tools (registry 33 to 36)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 3: Level-0 skills listing in the system prompt + agent addendum

**Files:**
- Modify: `crates/donna-core/src/ops.rs` (build_system_prompt injection), `crates/donna-core/src/agent.rs` (AGENT_SYSTEM_PROMPT_ADDENDUM)
- Test: inline (the injection is exercised by an existing prompt test if present; else assert skills_prompt_section from Task 1 — already tested. Add one ops-level test that build_system_prompt includes the listing when a skill exists.)

**Interfaces:**
- ops.rs build_system_prompt (mirror the Phase-4 memory injection at ~ops.rs:64-72): compute `let skills = knowledge_or_skills::skills_prompt_section()?;` (i.e. `skills::skills_prompt_section()`), inject it after the memory block and before `## What Donna knows about this user` — `if !skills.is_empty() { prompt.push_str(&skills); prompt.push_str("\n\n"); }`.
- agent.rs AGENT_SYSTEM_PROMPT_ADDENDUM (const ~agent.rs:33-38): append a sentence — "You have skills (listed under 'Available skills'). When a skill fits the task, call skill_view with its name to load its full instructions BEFORE acting, and follow its steps. If you work out a new repeatable multi-step recipe, consider skill_create to save it."

- [ ] **Step 1: Failing test**

```rust
// ops.rs test — build_system_prompt surfaces the skills listing
#[test]
fn system_prompt_lists_available_skills() {
    let db = test_db();
    let _g = skills_test_guard();
    crate::skills::save_skill("Trip Planner", "Plan a trip", "travel", "steps").unwrap();
    let cfg = load_config(&db).unwrap();
    let p = build_system_prompt(&cfg, None).unwrap();
    assert!(p.contains("## Available skills"));
    assert!(p.contains("Trip Planner"));
    assert!(!p.contains("steps")); // body not dumped into the prompt
}
```

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test -p donna-core && cargo check --workspace` (agent/server tests must still pass — the addendum change is additive text).
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Inject the available-skills catalog into the system prompt; agent uses skill_view

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 4: Skill suggestions — accept arm + review proposes skills

**Files:**
- Modify: `crates/donna-core/src/ops.rs` (suggestion_respond kind=="skill" arm + SkillSpec), `crates/donna-core/src/review.rs` (REVIEW_PROMPT skill shape)
- Test: inline in ops.rs

**Interfaces:**
- ops.rs: `#[derive(Deserialize)] pub struct SkillSpec { pub name: String, pub description: String, #[serde(default)] pub category: String, pub body: String }`.
- suggestion_respond accept path: mirror the routine arm's parse-before-resolve invariant. When `s.kind == "skill"`: parse `s.payload_json` as `SkillSpec`; None/malformed → notify "Suggestion couldn't be applied: invalid skill details" + return `Ok(format!("failed: ..."))` (suggestion STAYS pending, per the Phase-4 fix); valid → after `resolve_suggestion(id, "accepted")`, call `skills::save_skill(&spec.name, &spec.description, &category-or-"general", &spec.body)`, notify "Skill saved: <name>", return "accepted". (Keep the routine arm untouched; add the skill arm alongside it.)
- review.rs REVIEW_PROMPT: add a `kind:"skill"` example to the suggestions JSON shape and a rule: "Propose a skill (kind:'skill') when the SAME multi-step recipe recurs across the events/messages and isn't yet a saved skill; payload = {name, description, category, body} with body = the step-by-step instructions; dedup_key = 'skill:<kebab-name>'. Be conservative — only a genuinely repeated recipe, not a one-off." parse_suggestion already reads `kind` from JSON (defaulting "routine"); a skill suggestion must set `"kind":"skill"` explicitly (the prompt example makes this clear). The payload flows through unchanged as the SuggestionSpec.payload Value → serialized into payload_json.

- [ ] **Step 1: Failing tests**

```rust
#[tokio::test]
async fn suggestion_accept_skill_creates_it() {
    let db = test_db();
    let _g = skills_test_guard();
    let payload = serde_json::json!({"name":"Trip Planner","description":"Plan a trip","category":"travel","body":"1. dates\n2. book"}).to_string();
    let id = db.insert_suggestion("skill","Save Trip Planner skill","noticed you plan trips a lot",Some(&payload),"skill:trip-planner").unwrap().unwrap();
    let out = suggestion_respond(&db, id, true).await.unwrap();
    assert_eq!(out, "accepted");
    assert!(crate::skills::list_skills().unwrap().iter().any(|s| s.slug == "trip-planner"));
}
#[tokio::test]
async fn suggestion_accept_malformed_skill_stays_pending() {
    let db = test_db();
    let _g = skills_test_guard();
    let id = db.insert_suggestion("skill","bad","x",Some("{not json"),"skill:bad").unwrap().unwrap();
    let out = suggestion_respond(&db, id, true).await.unwrap();
    assert!(out.starts_with("failed"));
    assert_eq!(db.get_suggestion(id).unwrap().unwrap().status, "pending");
}
```

- [ ] **Step 2: RED** → **Step 3: Implement** → **Step 4: GREEN** — `cargo test --workspace && cargo check --workspace`.
- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "Author skills from accepted suggestions; nightly review can propose a skill

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 5: Skills UI + RPC + docs

**Files:**
- Modify: `donna-server/src/rpc.rs` (skills_list + skill_view arms), `src/lib/api.ts` (skill types + methods), `src/App.tsx` + `src/components/Sidebar.tsx` (route + nav link), `docs/ROADMAP.md`, `README.md` / `CONTEXT.md`
- Create: `src/routes/Skills.tsx`
- Verify: tsc/build + cargo test/check

**Interfaces:**
- rpc.rs: `"skills_list" => ok!(ops-or-skills::list…)` and `"skill_view" => ...(name, path?)` — thin arms returning the SkillMeta list / the skill body. (These call `donna_core::skills::list_skills()` / `view_skill` — expose a tiny ops wrapper if the rpc layer only calls ops:: by convention; check how other rpc arms are shaped and match.)
- api.ts: `Skill { name, slug, description, category }` + `skillsList(): Promise<Skill[]>`, `skillView(name, path?): Promise<string>`.
- Skills.tsx: a route listing the skill catalog (cards: name, category badge, description); clicking a skill fetches + renders its SKILL.md body (markdown). A small note: "Donna can create skills herself — accept a suggestion, or ask her in chat." (No manual create form required this phase — creation is via chat/suggestions; a read/browse view is the deliverable. If trivial, a "New skill" form calling a skill_create RPC arm is a nice-to-have, not required.)
- Sidebar + App.tsx: add a "Skills" nav item + `<Route path="/skills">` following the existing route/nav pattern (Dashboard/Chat/etc.).
- docs/ROADMAP.md: check the Phase 6 (Craft) items — and add a line noting all six spec phases are now shipped. README/CONTEXT: a short "Skills" paragraph (Donna discovers/uses/authors SKILL.md files; where they live; DONNA_SKILLS_DIR).

- [ ] **Step 1: Implement rpc arms + api.ts + Skills.tsx + nav + docs.** **Step 2:** `npx tsc --noEmit && npm run build && cargo test --workspace && cargo check --workspace` clean/green. **Step 3:** report a 4-line manual smoke checklist (open Skills, see the seeded skill, view its body; ask Donna in chat to use a skill).
- [ ] **Step 4: Commit and push**

```bash
git add -A
git commit -m "Add Skills browse page, skills RPC arms, and docs; Phase 6 complete

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

## Done criteria (whole phase)

1. `cargo test --workspace` green; tsc/build clean.
2. `skills.rs` persists SKILL.md files with a traversal guard; save→list→view roundtrips; a body never leaks into the catalog/listing.
3. `skills_list`/`skill_view`/`skill_create` are registered (count 36) and dispatch; the agent can list, load, and author skills.
4. Every system prompt carries a `## Available skills` name+description listing; the agent is told to `skill_view` before acting.
5. Accepting a `kind:"skill"` suggestion saves the skill (malformed payload stays pending); the nightly review can propose a skill for a recurring recipe.
6. A Skills page lists the catalog and renders a skill's SKILL.md.

## Follow-ups noted during planning (not in scope)

- Skills in the migration bundle (donna-core::bundle) so a desktop→server move carries them — skills are authored server-side post-migration, so low priority; note it.
- Agent-loop same-turn skill proposal (Hermes 5+-tool trigger) — nightly-review proposal is the provider-neutral, lower-latency choice; the same-turn hook can come later.
- Third-party skill hubs / install-from-URL (Hermes Skills Hub) — out of scope; local authoring only.
- Skill security scanning (Hermes scans installed skills for prompt injection) — only relevant once skills come from outside the user; local-authored skills don't need it yet.

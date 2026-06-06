# Knowledge Base

This folder is **Donna's local knowledge base** — the data behind the Mind Map view.

- **Each top-level folder is a category** (e.g. `About You`, `Routines`, `Feedback`,
  `People`, `Work`, `Study`, `Projects`).
- **Each file is a node.** Node files are Markdown with a little frontmatter and the
  description Donna wrote so she can recall it later:

  ```markdown
  ---
  label: Prefers morning meetings
  type: preference
  image:
  updated: 2026-06-06T08:00:00Z
  ---
  The user schedules deep work in the afternoon and prefers meetings before noon.
  ```

- **Sub-folders are branches** in the mind map (e.g. `Routines/Mornings/`).
- A node can have an **image** stored next to it in the same folder; the node's
  frontmatter `image:` field points to the file name.

## Privacy

Everything under `knowledge-base/` **except this README is gitignored**, so your personal
data is never pushed to GitHub. When someone clones Donna, this structure is recreated
locally on first run and holds only their own data.

You can edit these files by hand, or edit nodes from the Mind Map in the app — both stay
in sync because the app reads and writes these files directly.

import { useEffect, useState } from "react";
import {
  FolderOpen,
  Plus,
  Trash2,
  ExternalLink,
  FileText,
  Folder,
  Code2,
  BookOpen,
  LayoutGrid,
  Save,
  X,
} from "lucide-react";
import { Button, Badge, Card, EmptyState, Input, Spinner } from "../components/ui";
import { api, type Project, type ProjectFile } from "../lib/api";

const TEMPLATES = [
  {
    id: "coding",
    label: "Coding Project",
    icon: Code2,
    description: "README, .gitignore, src/ directory",
    color: "text-blue-400",
  },
  {
    id: "research",
    label: "Research Paper",
    icon: BookOpen,
    description: "Full paper structure with references tracker",
    color: "text-purple-400",
  },
  {
    id: "general",
    label: "General",
    icon: LayoutGrid,
    description: "Blank project folder",
    color: "text-donna-muted-light",
  },
] as const;

type TemplateId = (typeof TEMPLATES)[number]["id"];

export default function Projects() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeProjectId, setActiveProjectId] = useState<number | null>(null);
  const [files, setFiles] = useState<ProjectFile[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState("");
  const [fileLoading, setFileLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [showCreate, setShowCreate] = useState(false);

  // Create form
  const [newName, setNewName] = useState("");
  const [newPath, setNewPath] = useState("");
  const [newTemplate, setNewTemplate] = useState<TemplateId>("coding");
  const [creating, setCreating] = useState(false);

  const [error, setError] = useState<string | null>(null);

  const loadProjects = async () => {
    try {
      const list = await api.projectList();
      setProjects(list);
      if (list.length > 0 && activeProjectId === null) {
        setActiveProjectId(list[0].id);
      }
    } finally {
      setLoading(false);
    }
  };

  const loadFiles = async (projectId: number) => {
    try {
      const f = await api.projectListFiles(projectId);
      setFiles(f);
    } catch {
      setFiles([]);
    }
  };

  const openFile = async (projectId: number, path: string) => {
    if (files.find((f) => f.path === path)?.is_dir) return;
    setSelectedFile(path);
    setFileLoading(true);
    try {
      const content = await api.projectReadFile(projectId, path);
      setFileContent(content);
    } catch {
      setFileContent("");
    } finally {
      setFileLoading(false);
    }
  };

  const saveFile = async () => {
    if (!activeProjectId || !selectedFile) return;
    setSaving(true);
    try {
      await api.projectWriteFile(activeProjectId, selectedFile, fileContent);
    } finally {
      setSaving(false);
    }
  };

  const createProject = async () => {
    if (!newName.trim() || !newPath.trim()) return;
    setCreating(true);
    setError(null);
    try {
      const project = await api.projectCreate(newName.trim(), newTemplate, newPath.trim());
      setProjects((prev) => [project, ...prev]);
      setActiveProjectId(project.id);
      setShowCreate(false);
      setNewName("");
      setNewPath("");
      setNewTemplate("coding");
    } catch (e) {
      setError(String(e));
    } finally {
      setCreating(false);
    }
  };

  const deleteProject = async (id: number) => {
    await api.projectDelete(id);
    setProjects((prev) => prev.filter((p) => p.id !== id));
    if (activeProjectId === id) {
      setActiveProjectId(null);
      setFiles([]);
      setSelectedFile(null);
      setFileContent("");
    }
  };

  const openInEditor = async (path: string) => {
    await api.projectOpenInEditor(path).catch(() => {});
  };

  useEffect(() => {
    loadProjects();
  }, []);

  useEffect(() => {
    if (activeProjectId) {
      loadFiles(activeProjectId);
      setSelectedFile(null);
      setFileContent("");
    }
  }, [activeProjectId]);

  const activeProject = projects.find((p) => p.id === activeProjectId);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Project list sidebar */}
      <div className="flex w-64 flex-col border-r border-donna-border bg-donna-panel">
        <div className="flex h-14 items-center justify-between border-b border-donna-border px-4">
          <span className="text-xs font-semibold uppercase tracking-widest text-donna-muted">
            Projects
          </span>
          <button
            onClick={() => setShowCreate(true)}
            className="rounded p-1 text-donna-muted hover:bg-donna-surface-hover hover:text-donna-text transition-colors"
            title="New project"
          >
            <Plus size={14} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-2">
          {projects.length === 0 && (
            <EmptyState
              icon={<FolderOpen size={24} />}
              title="No projects yet"
              description="Create a project to get started"
              action={
                <Button size="sm" onClick={() => setShowCreate(true)}>
                  <Plus size={12} /> New Project
                </Button>
              }
            />
          )}
          {projects.map((p) => {
            const tmpl = TEMPLATES.find((t) => t.id === p.template);
            const TmplIcon = tmpl?.icon ?? FolderOpen;
            return (
              <div
                key={p.id}
                className={`group mb-0.5 flex cursor-pointer items-center gap-2.5 rounded px-2.5 py-2 transition-colors ${
                  activeProjectId === p.id
                    ? "bg-donna-accent-dim text-donna-accent-light"
                    : "text-donna-muted-light hover:bg-donna-surface-hover hover:text-donna-text"
                }`}
                onClick={() => setActiveProjectId(p.id)}
              >
                <TmplIcon size={14} className={activeProjectId === p.id ? "" : (tmpl?.color ?? "")} />
                <span className="flex-1 truncate text-sm">{p.name}</span>
                <button
                  onClick={(e) => { e.stopPropagation(); deleteProject(p.id); }}
                  className="opacity-0 transition-opacity group-hover:opacity-100 text-donna-muted hover:text-red-400"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            );
          })}
        </div>
      </div>

      {/* File tree */}
      {activeProject && (
        <div className="flex w-56 flex-col border-r border-donna-border bg-donna-panel">
          <div className="flex h-14 items-center justify-between border-b border-donna-border px-3">
            <span className="truncate text-xs font-medium text-donna-text-secondary">{activeProject.name}</span>
            <button
              onClick={() => openInEditor(activeProject.path)}
              className="rounded p-1 text-donna-muted hover:text-donna-text transition-colors"
              title="Open in editor"
            >
              <ExternalLink size={12} />
            </button>
          </div>
          <div className="flex-1 overflow-y-auto p-2">
            {files.length === 0 && (
              <p className="px-2 py-4 text-xs text-donna-muted">No files yet.</p>
            )}
            {files.map((f) => (
              <button
                key={f.path}
                onClick={() => activeProjectId && openFile(activeProjectId, f.path)}
                className={`flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs transition-colors ${
                  selectedFile === f.path
                    ? "bg-donna-accent-dim text-donna-accent-light"
                    : "text-donna-muted-light hover:bg-donna-surface-hover hover:text-donna-text"
                } ${f.is_dir ? "cursor-default" : ""}`}
                style={{ paddingLeft: `${(f.path.split("/").length - 1) * 10 + 8}px` }}
              >
                {f.is_dir ? (
                  <Folder size={11} className="shrink-0 text-donna-muted" />
                ) : (
                  <FileText size={11} className="shrink-0" />
                )}
                <span className="truncate">{f.name}</span>
              </button>
            ))}
          </div>
          {selectedFile && (
            <div className="border-t border-donna-border p-2">
              <Badge variant="accent">{activeProject.template}</Badge>
            </div>
          )}
        </div>
      )}

      {/* Editor / main content */}
      <div className="flex flex-1 flex-col overflow-hidden">
        {!activeProject ? (
          <div className="flex h-full flex-col items-center justify-center gap-6 p-8">
            <div className="text-center">
              <FolderOpen size={40} className="mx-auto mb-3 text-donna-muted" />
              <p className="text-base font-medium text-donna-text">Select or create a project</p>
              <p className="mt-1 text-sm text-donna-muted">
                Manage coding projects, research papers, and more
              </p>
            </div>

            <div className="grid w-full max-w-lg grid-cols-3 gap-3">
              {TEMPLATES.map((tmpl) => (
                <Card
                  key={tmpl.id}
                  className="cursor-pointer p-4 text-center transition-colors hover:border-donna-border-strong hover:bg-donna-surface-raised"
                  onClick={() => { setNewTemplate(tmpl.id); setShowCreate(true); }}
                >
                  <tmpl.icon size={20} className={`mx-auto mb-2 ${tmpl.color}`} />
                  <p className="text-xs font-medium text-donna-text">{tmpl.label}</p>
                  <p className="mt-0.5 text-[10px] text-donna-muted">{tmpl.description}</p>
                </Card>
              ))}
            </div>
          </div>
        ) : selectedFile ? (
          <>
            <div className="flex h-14 items-center justify-between border-b border-donna-border px-4">
              <div className="flex items-center gap-2 text-sm">
                <FileText size={14} className="text-donna-muted" />
                <span className="font-mono text-donna-text-secondary">{selectedFile}</span>
              </div>
              <div className="flex items-center gap-2">
                <Button size="sm" variant="ghost" onClick={() => { setSelectedFile(null); setFileContent(""); }}>
                  <X size={12} /> Close
                </Button>
                <Button size="sm" onClick={saveFile} disabled={saving}>
                  {saving ? <Spinner /> : <Save size={12} />}
                  Save
                </Button>
              </div>
            </div>
            {fileLoading ? (
              <div className="flex flex-1 items-center justify-center">
                <Spinner />
              </div>
            ) : (
              <textarea
                value={fileContent}
                onChange={(e) => setFileContent(e.target.value)}
                onKeyDown={(e) => { if ((e.metaKey || e.ctrlKey) && e.key === "s") { e.preventDefault(); saveFile(); } }}
                className="flex-1 resize-none bg-donna-bg p-6 font-mono text-sm text-donna-text outline-none leading-relaxed"
                spellCheck={false}
              />
            )}
          </>
        ) : (
          <div className="flex h-full flex-col">
            <div className="flex h-14 items-center justify-between border-b border-donna-border px-4">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-donna-text">{activeProject.name}</span>
                <Badge variant={activeProject.template === "research" ? "accent" : "default"}>
                  {activeProject.template}
                </Badge>
              </div>
              <div className="flex gap-2">
                <Button size="sm" variant="ghost" onClick={() => openInEditor(activeProject.path)}>
                  <ExternalLink size={12} /> Open in Editor
                </Button>
                <Button size="sm" variant="ghost" onClick={async () => {
                  if (!activeProject) return;
                  try {
                    await api.projectStatusReport(activeProject.id);
                    // Report saved as doc + notification
                  } catch {
                    // ignore
                  }
                }}>
                  <FileText size={12} /> Status Report
                </Button>
              </div>
            </div>
            <div className="flex-1 overflow-y-auto p-6">
              <div className="mx-auto max-w-lg">
                <p className="mb-4 text-xs font-semibold uppercase tracking-widest text-donna-muted">
                  Files
                </p>
                <div className="space-y-1">
                  {files.filter(f => !f.is_dir).map((f) => (
                    <button
                      key={f.path}
                      onClick={() => activeProjectId && openFile(activeProjectId, f.path)}
                      className="flex w-full items-center gap-3 rounded px-3 py-2.5 text-sm text-donna-text-secondary transition-colors hover:bg-donna-surface-hover hover:text-donna-text"
                    >
                      <FileText size={14} className="shrink-0 text-donna-muted" />
                      <span className="font-mono text-xs">{f.path}</span>
                    </button>
                  ))}
                </div>
                {activeProject.template === "research" && (
                  <div className="mt-8">
                    <p className="mb-3 text-xs font-semibold uppercase tracking-widest text-donna-muted">
                      Research Tools
                    </p>
                    <div className="space-y-2">
                      <Card className="p-4">
                        <p className="text-sm font-medium text-donna-text">Paper Sections</p>
                        <p className="mt-0.5 text-xs text-donna-muted">Open paper.md to write your sections</p>
                      </Card>
                      <Card className="p-4">
                        <p className="text-sm font-medium text-donna-text">References</p>
                        <p className="mt-0.5 text-xs text-donna-muted">Track sources in references.md</p>
                      </Card>
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Create project modal */}
      {showCreate && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <Card className="w-full max-w-md p-6 shadow-elevated">
            <div className="mb-5 flex items-center justify-between">
              <h2 className="text-base font-semibold text-donna-text">New Project</h2>
              <button onClick={() => setShowCreate(false)} className="text-donna-muted hover:text-donna-text">
                <X size={18} />
              </button>
            </div>

            <div className="space-y-4">
              <div>
                <label className="mb-1.5 block text-xs font-medium text-donna-text-secondary">Project name</label>
                <Input
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  placeholder="My awesome project"
                  autoFocus
                />
              </div>

              <div>
                <label className="mb-1.5 block text-xs font-medium text-donna-text-secondary">Location (absolute path)</label>
                <Input
                  value={newPath}
                  onChange={(e) => setNewPath(e.target.value)}
                  placeholder="/Users/you/Documents/projects/my-project"
                  className="font-mono text-xs"
                />
              </div>

              <div>
                <label className="mb-2 block text-xs font-medium text-donna-text-secondary">Template</label>
                <div className="grid grid-cols-3 gap-2">
                  {TEMPLATES.map((tmpl) => (
                    <button
                      key={tmpl.id}
                      onClick={() => setNewTemplate(tmpl.id)}
                      className={`rounded border p-3 text-center transition-colors ${
                        newTemplate === tmpl.id
                          ? "border-donna-accent/40 bg-donna-accent-dim text-donna-accent-light"
                          : "border-donna-border text-donna-muted hover:border-donna-border-strong hover:text-donna-text-secondary"
                      }`}
                    >
                      <tmpl.icon size={16} className="mx-auto mb-1.5" />
                      <p className="text-[10px] font-medium">{tmpl.label}</p>
                    </button>
                  ))}
                </div>
              </div>

              {error && (
                <p className="rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-xs text-red-400">
                  {error}
                </p>
              )}

              <div className="flex justify-end gap-2 pt-1">
                <Button variant="ghost" onClick={() => setShowCreate(false)}>Cancel</Button>
                <Button onClick={createProject} disabled={creating || !newName.trim() || !newPath.trim()}>
                  {creating ? <Spinner /> : <Plus size={14} />}
                  Create
                </Button>
              </div>
            </div>
          </Card>
        </div>
      )}
    </div>
  );
}

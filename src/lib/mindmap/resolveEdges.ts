import type { KgEdge, KgGraph } from "../api";

function folderId(path: string[]): string {
  return `folder:${path.join("/")}`;
}

/** Client-side fallback when the API returns no edges (older data). */
function inferHierarchyEdges(graph: KgGraph): KgEdge[] {
  const ids = new Set(graph.nodes.map((n) => n.id));
  const edges: KgEdge[] = [];
  const seen = new Set<string>();

  const add = (source: string, target: string) => {
    const key = `${source}->${target}`;
    if (source === target || seen.has(key) || !ids.has(source) || !ids.has(target)) return;
    seen.add(key);
    edges.push({ source, target });
  };

  for (const node of graph.nodes) {
    if (node.type === "folder") {
      if (node.folder.length > 1) {
        const parent = folderId(node.folder.slice(0, -1));
        add(parent, node.id);
      }
      continue;
    }
    add(folderId(node.folder), node.id);
  }

  return edges;
}

/** Use hierarchy edges from the API; fall back to inferring parent → child links. */
export function resolveGraphEdges(graph: KgGraph): KgEdge[] {
  const ids = new Set(graph.nodes.map((n) => n.id));
  const stored = graph.edges.filter((e) => ids.has(e.source) && ids.has(e.target));
  if (stored.length > 0) return stored;
  return inferHierarchyEdges(graph);
}

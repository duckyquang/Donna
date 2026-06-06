import type { KgEdge, KgGraph } from "../api";

function inferGroupEdges(graph: KgGraph): KgEdge[] {
  const byGroup = new Map<string, string[]>();
  for (const n of graph.nodes) {
    const g = n.group || "Topics";
    if (!byGroup.has(g)) byGroup.set(g, []);
    byGroup.get(g)!.push(n.id);
  }

  const inferred: KgEdge[] = [];
  for (const ids of byGroup.values()) {
    if (ids.length < 2) continue;
    const hub = ids[0];
    for (let i = 1; i < ids.length; i++) {
      inferred.push({ source: hub, target: ids[i]! });
    }
  }
  return inferred;
}

/**
 * Prefer stored edges that reference existing nodes. If none remain, link nodes
 * in the same group so the map still shows connections.
 */
export function resolveGraphEdges(graph: KgGraph): KgEdge[] {
  const ids = new Set(graph.nodes.map((n) => n.id));
  const stored = graph.edges.filter((e) => ids.has(e.source) && ids.has(e.target));
  if (stored.length > 0) return stored;
  return inferGroupEdges(graph);
}

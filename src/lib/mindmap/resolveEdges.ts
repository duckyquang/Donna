import type { KgEdge, KgGraph } from "../api";

/**
 * Use stored edges when present. Otherwise link nodes in the same group so the
 * map still shows connections (e.g. two People nodes with no explicit edge yet).
 */
export function resolveGraphEdges(graph: KgGraph): KgEdge[] {
  if (graph.edges.length > 0) return graph.edges;

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

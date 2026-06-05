import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  type Node,
  type Edge,
  type NodeMouseHandler,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { RefreshCw, X } from "lucide-react";
import { api, type KgGraph, type KgNode } from "../lib/api";
import { Spinner } from "../components/ui";

// Distinct, theme-friendly colors per knowledge cluster.
const GROUP_COLORS: Record<string, string> = {
  People: "#e8a55a",
  Projects: "#5b9bd5",
  Preferences: "#a78bfa",
  Routines: "#4ade80",
  Places: "#f472b6",
  Health: "#f87171",
  Topics: "#facc15",
};

function colorFor(group: string): string {
  if (GROUP_COLORS[group]) return GROUP_COLORS[group];
  // Stable fallback hue from the group name.
  let hash = 0;
  for (let i = 0; i < group.length; i++) hash = group.charCodeAt(i) + ((hash << 5) - hash);
  return `hsl(${Math.abs(hash) % 360} 60% 60%)`;
}

/**
 * Build a clustered layout: group nodes into chunks, arrange clusters around a circle,
 * and lay each cluster's nodes in a ring around a group label. Deterministic so the map
 * is stable across refreshes.
 */
function buildLayout(graph: KgGraph): { nodes: Node[]; edges: Edge[] } {
  const byGroup = new Map<string, KgNode[]>();
  for (const n of graph.nodes) {
    const g = n.group || "Topics";
    if (!byGroup.has(g)) byGroup.set(g, []);
    byGroup.get(g)!.push(n);
  }

  const groups = [...byGroup.keys()].sort();
  const nodes: Node[] = [];
  const clusterRadius = 420;

  groups.forEach((group, gi) => {
    const color = colorFor(group);
    const groupAngle = (2 * Math.PI * gi) / Math.max(groups.length, 1);
    const cx = Math.cos(groupAngle) * clusterRadius;
    const cy = Math.sin(groupAngle) * clusterRadius;
    const members = byGroup.get(group)!;
    const ring = Math.max(110, members.length * 26);

    // Cluster label at the center of the chunk.
    nodes.push({
      id: `group:${group}`,
      position: { x: cx, y: cy },
      data: { label: `${group} · ${members.length}` },
      selectable: false,
      draggable: false,
      style: {
        background: "transparent",
        border: "none",
        color,
        fontWeight: 700,
        fontSize: 14,
        width: 160,
        textAlign: "center" as const,
        boxShadow: "none",
      },
    });

    members.forEach((m, mi) => {
      const a = (2 * Math.PI * mi) / Math.max(members.length, 1);
      nodes.push({
        id: m.id,
        position: { x: cx + Math.cos(a) * ring, y: cy + Math.sin(a) * ring },
        data: { label: m.label },
        style: {
          background: `${color}22`,
          border: `1px solid ${color}`,
          color: "#f3f4f6",
          borderRadius: 12,
          padding: "6px 12px",
          fontSize: 12,
          width: "auto",
          maxWidth: 180,
        },
      });
    });
  });

  const ids = new Set(graph.nodes.map((n) => n.id));
  const edges: Edge[] = graph.edges
    .filter((e) => ids.has(e.source) && ids.has(e.target))
    .map((e) => ({
      id: `${e.source}->${e.target}`,
      source: e.source,
      target: e.target,
      style: { stroke: "#ffffff22" },
    }));

  return { nodes, edges };
}

export default function MindMap() {
  const [graph, setGraph] = useState<KgGraph>({ nodes: [], edges: [] });
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<KgNode | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setGraph(await api.kgGraph());
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const { nodes, edges } = useMemo(() => buildLayout(graph), [graph]);
  const nodeById = useMemo(
    () => new Map(graph.nodes.map((n) => [n.id, n])),
    [graph]
  );

  const onNodeClick: NodeMouseHandler = (_e, node) => {
    const found = nodeById.get(node.id);
    setSelected(found ?? null);
  };

  const groupsPresent = useMemo(
    () => [...new Set(graph.nodes.map((n) => n.group || "Topics"))].sort(),
    [graph]
  );

  return (
    <div className="relative h-full w-full bg-donna-bg">
      <header className="absolute left-0 right-0 top-0 z-10 flex items-center justify-between border-b border-white/10 bg-donna-bg/80 px-6 py-3 backdrop-blur">
        <div>
          <h1 className="text-sm font-semibold text-white">Mind Map</h1>
          <p className="text-xs text-gray-500">
            Donna&apos;s living map of what she knows about you · {graph.nodes.length}{" "}
            nodes
          </p>
        </div>
        <button
          onClick={load}
          className="flex items-center gap-2 rounded-lg border border-white/15 px-3 py-1.5 text-xs text-gray-200 hover:bg-white/5"
        >
          {loading ? <Spinner /> : <RefreshCw size={14} />}
          Refresh
        </button>
      </header>

      {/* Legend */}
      {groupsPresent.length > 0 && (
        <div className="absolute bottom-4 left-4 z-10 flex flex-wrap gap-2 rounded-xl border border-white/10 bg-donna-surface/90 p-3 backdrop-blur">
          {groupsPresent.map((g) => (
            <span key={g} className="flex items-center gap-1.5 text-xs text-gray-300">
              <span
                className="h-2.5 w-2.5 rounded-full"
                style={{ background: colorFor(g) }}
              />
              {g}
            </span>
          ))}
        </div>
      )}

      {graph.nodes.length === 0 && !loading ? (
        <div className="flex h-full items-center justify-center px-6 text-center">
          <div className="max-w-md">
            <p className="text-sm text-gray-300">
              Donna hasn&apos;t mapped anything yet.
            </p>
            <p className="mt-1 text-xs text-gray-500">
              Chat with her and tell her about yourself — your people, projects, and
              routines. She&apos;ll build this map automatically as you talk.
            </p>
          </div>
        </div>
      ) : (
        <div className="mindmap h-full w-full">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodeClick={onNodeClick}
            nodesConnectable={false}
            nodesDraggable={false}
            fitView
            proOptions={{ hideAttribution: true }}
            minZoom={0.2}
          >
            <Background color="#ffffff10" gap={24} />
            <Controls showInteractive={false} />
          </ReactFlow>
        </div>
      )}

      {/* Node note panel */}
      {selected && (
        <div className="absolute right-4 top-20 z-10 w-72 rounded-xl border border-white/10 bg-donna-surface p-4 shadow-xl">
          <div className="mb-2 flex items-start justify-between gap-2">
            <div>
              <div className="text-sm font-semibold text-white">{selected.label}</div>
              <span
                className="mt-1 inline-block rounded-full px-2 py-0.5 text-[10px]"
                style={{
                  background: `${colorFor(selected.group)}22`,
                  color: colorFor(selected.group),
                }}
              >
                {selected.group}
              </span>
            </div>
            <button
              onClick={() => setSelected(null)}
              className="text-gray-500 hover:text-white"
            >
              <X size={16} />
            </button>
          </div>
          <p className="text-xs leading-relaxed text-gray-300">
            {selected.note || "No note yet."}
          </p>
          <p className="mt-3 text-[10px] text-gray-600">
            Noted by Donna · updated {selected.updatedAt.slice(0, 10)}
          </p>
        </div>
      )}
    </div>
  );
}

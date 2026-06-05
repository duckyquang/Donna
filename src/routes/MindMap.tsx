import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  type Node,
  type Edge,
  type NodeMouseHandler,
  type NodeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { RefreshCw, X } from "lucide-react";
import { KgCircleNode } from "../components/mindmap/KgCircleNode";
import { api, type KgGraph, type KgNode } from "../lib/api";
import { connectionCount, forceLayout } from "../lib/mindmap/forceLayout";
import { Spinner } from "../components/ui";

const nodeTypes: NodeTypes = { kgCircle: KgCircleNode };

const GROUP_COLORS: Record<string, string> = {
  People: "#e8a55a",
  Projects: "#5b9bd5",
  Preferences: "#c9742a",
  Routines: "#4ade80",
  Places: "#f472b6",
  Health: "#f87171",
  Topics: "#facc15",
};

function colorFor(group: string): string {
  if (GROUP_COLORS[group]) return GROUP_COLORS[group];
  let hash = 0;
  for (let i = 0; i < group.length; i++) hash = group.charCodeAt(i) + ((hash << 5) - hash);
  return `hsl(${Math.abs(hash) % 360} 55% 55%)`;
}

function nodeSize(id: string, edges: KgGraph["edges"]): number {
  const links = connectionCount(id, edges);
  return 10 + Math.min(links, 10) * 1.8;
}

function buildGraphLayout(graph: KgGraph): { nodes: Node[]; edges: Edge[] } {
  const positions = forceLayout(graph.nodes, graph.edges);
  const ids = new Set(graph.nodes.map((n) => n.id));

  const nodes: Node[] = graph.nodes.map((m) => {
    const size = nodeSize(m.id, graph.edges);
    const pos = positions.get(m.id) ?? { x: 0, y: 0 };
    return {
      id: m.id,
      type: "kgCircle",
      position: { x: pos.x - size / 2, y: pos.y - size / 2 },
      data: {
        label: m.label,
        color: colorFor(m.group || "Topics"),
        size,
      },
      draggable: false,
      selectable: true,
    };
  });

  const edges: Edge[] = graph.edges
    .filter((e) => ids.has(e.source) && ids.has(e.target))
    .map((e) => ({
      id: `${e.source}->${e.target}`,
      source: e.source,
      target: e.target,
      type: "straight",
      style: { stroke: "#ffffff28", strokeWidth: 1 },
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

  const { nodes, edges } = useMemo(() => buildGraphLayout(graph), [graph]);
  const nodeById = useMemo(
    () => new Map(graph.nodes.map((n) => [n.id, n])),
    [graph]
  );

  const onNodeClick: NodeMouseHandler = (_e, node) => {
    const found = nodeById.get(node.id);
    if (found) setSelected(found);
  };

  const onPaneClick = () => setSelected(null);

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
            nodes · click a node to read its note
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
            nodeTypes={nodeTypes}
            onNodeClick={onNodeClick}
            onPaneClick={onPaneClick}
            nodesConnectable={false}
            nodesDraggable={false}
            elementsSelectable
            fitView
            fitViewOptions={{ padding: 0.3 }}
            proOptions={{ hideAttribution: true }}
            minZoom={0.15}
            maxZoom={2.5}
          >
            <Background color="#ffffff08" gap={32} />
            <Controls showInteractive={false} />
          </ReactFlow>
        </div>
      )}

      {selected && (
        <>
          <button
            type="button"
            aria-label="Close"
            className="absolute inset-0 z-20 bg-black/55 backdrop-blur-[2px]"
            onClick={() => setSelected(null)}
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="node-popup-title"
            className="absolute left-1/2 top-1/2 z-30 w-full max-w-md -translate-x-1/2 -translate-y-1/2 px-4"
          >
            <div className="rounded-2xl border border-white/10 bg-donna-surface p-5 shadow-2xl">
              <div className="mb-4 flex items-start gap-3">
                <span
                  className="mt-1 h-4 w-4 shrink-0 rounded-full"
                  style={{ background: colorFor(selected.group) }}
                />
                <div className="min-w-0 flex-1">
                  <h2
                    id="node-popup-title"
                    className="text-base font-semibold text-white"
                  >
                    {selected.label}
                  </h2>
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
                  className="shrink-0 rounded-lg p-1 text-gray-500 hover:bg-white/5 hover:text-white"
                >
                  <X size={18} />
                </button>
              </div>
              <div className="rounded-xl border border-white/10 bg-donna-bg px-4 py-3">
                <p className="text-sm leading-relaxed text-gray-200">
                  {selected.note || "No note yet."}
                </p>
              </div>
              <p className="mt-3 text-[10px] text-gray-600">
                Noted by Donna · updated {selected.updatedAt.slice(0, 10)}
              </p>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

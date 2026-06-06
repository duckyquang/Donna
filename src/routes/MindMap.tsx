import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  useNodesState,
  type Node,
  type NodeMouseHandler,
  type NodeTypes,
  type OnNodeDrag,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Plus, RefreshCw, Trash2 } from "lucide-react";
import { KgCircleNode, type KgCircleNodeData } from "../components/mindmap/KgCircleNode";
import { MindMapGraphLinks } from "../components/mindmap/MindMapGraphLinks";
import { NodeDetailPanel } from "../components/mindmap/NodeDetailPanel";
import { NodeEditor } from "../components/mindmap/NodeEditor";
import { api, type KgGraph, type KgEdge, type KgNode } from "../lib/api";
import { ForceSim, connectionCount, forceLayout } from "../lib/mindmap/forceLayout";
import { resolveGraphEdges } from "../lib/mindmap/resolveEdges";
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

function topCategory(group: string): string {
  return (group.split(" / ")[0] ?? group).trim();
}

function colorFor(group: string): string {
  const top = topCategory(group);
  if (GROUP_COLORS[top]) return GROUP_COLORS[top];
  let hash = 0;
  for (let i = 0; i < top.length; i++) hash = top.charCodeAt(i) + ((hash << 5) - hash);
  return `hsl(${Math.abs(hash) % 360} 55% 55%)`;
}

function nodeSize(node: KgNode, edges: KgEdge[]): number {
  const links = connectionCount(node.id, edges);
  if (node.type === "folder") {
    return 14 + Math.min(links, 12) * 1.4;
  }
  return 10 + Math.min(links, 10) * 1.8;
}

function nodeDimensions(d: KgCircleNodeData) {
  if (d.isFolder) {
    const w = Math.max(d.size * 1.6, 36);
    const h = Math.max(d.size * 0.9, 22);
    return { w, h };
  }
  const s = d.size;
  return { w: s, h: s };
}

function centerToPosition(cx: number, cy: number, w: number, h: number) {
  return { x: cx - w / 2, y: cy - h / 2 };
}

function buildFlowNodes(
  graph: KgGraph,
  resolvedEdges: KgEdge[],
  positions: Map<string, { x: number; y: number }>
): Node[] {
  return graph.nodes.map((m) => {
    const size = nodeSize(m, resolvedEdges);
    const isFolder = m.type === "folder";
    const pos = positions.get(m.id) ?? { x: 0, y: 0 };
    const w = isFolder ? Math.max(size * 1.6, 36) : size;
    const h = isFolder ? Math.max(size * 0.9, 22) : size;
    return {
      id: m.id,
      type: "kgCircle",
      position: centerToPosition(pos.x, pos.y, w, h),
      initialWidth: w,
      initialHeight: h,
      width: w,
      height: h,
      data: {
        label: m.label,
        color: colorFor(m.folder[0] ?? m.group ?? "Topics"),
        size,
        isFolder,
      } satisfies KgCircleNodeData,
      draggable: true,
      selectable: true,
    };
  });
}

function applySimPositions(nodes: Node[], sim: ForceSim): Node[] {
  const positions = sim.positions();
  return nodes.map((n) => {
    const { w, h } = nodeDimensions(n.data as KgCircleNodeData);
    const pos = positions.get(n.id) ?? { x: 0, y: 0 };
    return { ...n, position: centerToPosition(pos.x, pos.y, w, h) };
  });
}

export default function MindMap() {
  const [graph, setGraph] = useState<KgGraph>({ nodes: [], edges: [] });
  const [loading, setLoading] = useState(true);
  const [resetting, setResetting] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editing, setEditing] = useState<KgNode | "new" | null>(null);
  const [rfNodes, setRfNodes, onNodesChange] = useNodesState<Node>([]);
  const simRef = useRef<ForceSim | null>(null);
  const didDragRef = useRef(false);

  const resolvedEdges = useMemo(
    () => (graph.nodes.length > 0 ? resolveGraphEdges(graph) : []),
    [graph]
  );

  const nodeColorById = useMemo(
    () => new Map(graph.nodes.map((n) => [n.id, colorFor(n.group || "Topics")])),
    [graph.nodes]
  );

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setGraph(await api.kgGraph());
    } finally {
      setLoading(false);
    }
  }, []);

  const resetKnowledge = useCallback(async () => {
    setResetting(true);
    try {
      await api.kgReset();
      setEditing(null);
      setShowResetConfirm(false);
      setGraph({ nodes: [], edges: [] });
    } finally {
      setResetting(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    if (graph.nodes.length === 0) {
      setRfNodes([]);
      simRef.current = null;
      return;
    }
    const layout = forceLayout(graph.nodes, resolvedEdges);
    simRef.current = ForceSim.fromLayout(
      graph.nodes.map((n) => n.id),
      resolvedEdges,
      layout
    );
    setRfNodes(buildFlowNodes(graph, resolvedEdges, layout));
  }, [graph, resolvedEdges, setRfNodes]);

  const nodeById = useMemo(
    () => new Map(graph.nodes.map((n) => [n.id, n])),
    [graph]
  );

  const selectedNode = selectedId ? nodeById.get(selectedId) ?? null : null;

  useEffect(() => {
    if (selectedId && !nodeById.has(selectedId)) {
      setSelectedId(null);
    }
  }, [selectedId, nodeById]);

  const onNodeDragStart: OnNodeDrag = () => {
    didDragRef.current = false;
  };

  const onNodeDrag: OnNodeDrag = (_e, node) => {
    didDragRef.current = true;
    const sim = simRef.current;
    if (!sim) return;

    const w = node.width ?? (node.data as KgCircleNodeData).size;
    const h = node.height ?? (node.data as KgCircleNodeData).size;
    const cx = node.position.x + w / 2;
    const cy = node.position.y + h / 2;
    for (let i = 0; i < 5; i++) {
      sim.tick({ id: node.id, x: cx, y: cy });
    }

    setRfNodes((nds) => applySimPositions(nds, sim));
  };

  const onNodeDragStop: OnNodeDrag = (_e, node) => {
    const sim = simRef.current;
    if (!sim) return;

    const w = node.width ?? (node.data as KgCircleNodeData).size;
    const h = node.height ?? (node.data as KgCircleNodeData).size;
    const cx = node.position.x + w / 2;
    const cy = node.position.y + h / 2;
    for (let i = 0; i < 12; i++) {
      sim.tick({ id: node.id, x: cx, y: cy }, 0.6);
    }
    setRfNodes((nds) => applySimPositions(nds, sim));

    // Reset after drag so the next tap opens details instead of being blocked.
    window.setTimeout(() => {
      didDragRef.current = false;
    }, 0);
  };

  const onNodeClick: NodeMouseHandler = (e, node) => {
    e.stopPropagation();
    if (didDragRef.current) return;
    setSelectedId(node.id);
    setEditing(null);
    setRfNodes((nds) =>
      nds.map((n) => ({ ...n, selected: n.id === node.id }))
    );
  };

  const onPaneClick = () => {
    setSelectedId(null);
    setEditing(null);
    setRfNodes((nds) => nds.map((n) => ({ ...n, selected: false })));
  };

  const groupsPresent = useMemo(
    () =>
      [
        ...new Set(
          graph.nodes.map((n) => topCategory(n.folder[0] ?? n.group ?? "Topics"))
        ),
      ].sort(),
    [graph]
  );

  const childNodes = useMemo(() => {
    if (!selectedId) return [];
    const childIds = new Set(
      resolvedEdges.filter((e) => e.source === selectedId).map((e) => e.target)
    );
    return graph.nodes.filter((n) => childIds.has(n.id));
  }, [selectedId, graph.nodes, resolvedEdges]);

  return (
    <div className="relative h-full w-full bg-donna-bg">
      <header className="absolute left-0 right-0 top-0 z-10 flex items-center justify-between border-b border-white/10 bg-donna-bg/80 px-6 py-3 backdrop-blur">
        <div>
          <h1 className="text-sm font-semibold text-white">Mind Map</h1>
          <p className="text-xs text-gray-500">
            Donna&apos;s living map of what she knows about you · {graph.nodes.length}{" "}
            nodes · drag to rearrange · click a node to read its details
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setEditing("new")}
            className="flex items-center gap-2 rounded-lg bg-donna-accent px-3 py-1.5 text-xs font-medium text-white hover:bg-donna-accent-hover"
          >
            <Plus size={14} />
            Add node
          </button>
          <button
            type="button"
            onClick={() => setShowResetConfirm(true)}
            disabled={graph.nodes.length === 0 || loading || resetting}
            className="flex items-center gap-2 rounded-lg border border-red-500/30 px-3 py-1.5 text-xs text-red-300 hover:bg-red-500/10 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Trash2 size={14} />
            Reset knowledge
          </button>
          <button
            onClick={load}
            disabled={loading || resetting}
            className="flex items-center gap-2 rounded-lg border border-white/15 px-3 py-1.5 text-xs text-gray-200 hover:bg-white/5 disabled:opacity-40"
          >
            {loading ? <Spinner /> : <RefreshCw size={14} />}
            Refresh
          </button>
        </div>
      </header>

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
        <div className="flex h-full w-full pt-[52px] transition-[padding] duration-300 ease-out">
          <div className="mindmap relative min-w-0 flex-1 transition-[flex-grow] duration-300 ease-out">
            {groupsPresent.length > 0 && (
              <div className="pointer-events-none absolute bottom-4 right-4 z-10 flex flex-col gap-2 rounded-xl border border-white/10 bg-donna-surface/90 p-3 backdrop-blur">
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
            <ReactFlow
              nodes={rfNodes}
              edges={[]}
              onNodesChange={onNodesChange}
              nodeTypes={nodeTypes}
              onNodeClick={onNodeClick}
              onNodeDragStart={onNodeDragStart}
              onNodeDrag={onNodeDrag}
              onNodeDragStop={onNodeDragStop}
              onPaneClick={onPaneClick}
              nodesConnectable={false}
              nodesDraggable
              nodeDragThreshold={8}
              selectionOnDrag={false}
              selectNodesOnDrag={false}
              panOnDrag
              elementsSelectable
              fitView
            fitViewOptions={{ padding: 0.3 }}
            proOptions={{ hideAttribution: true }}
            minZoom={0.15}
            maxZoom={2.5}
          >
            <MindMapGraphLinks
              edges={resolvedEdges}
              colorForNode={(id) => nodeColorById.get(id) ?? "#e8a55a"}
            />
            <Background color="#ffffff08" gap={32} />
            <Controls showInteractive={false} />
            </ReactFlow>
          </div>

          {selectedNode && (
            <NodeDetailPanel
              node={selectedNode}
              childNodes={childNodes}
              onClose={() => setSelectedId(null)}
              onEdit={() => {
                if (selectedNode.type !== "folder") setEditing(selectedNode);
              }}
              onSelectChild={(id) => {
                setSelectedId(id);
                setRfNodes((nds) =>
                  nds.map((n) => ({ ...n, selected: n.id === id }))
                );
              }}
              onDeleted={() => {
                setSelectedId(null);
                load();
              }}
            />
          )}
        </div>
      )}

      {showResetConfirm && (
        <>
          <button
            type="button"
            aria-label="Cancel reset"
            className="absolute inset-0 z-20 bg-black/55 backdrop-blur-[2px]"
            onClick={() => !resetting && setShowResetConfirm(false)}
          />
          <div
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="reset-knowledge-title"
            aria-describedby="reset-knowledge-desc"
            className="absolute left-1/2 top-1/2 z-30 w-full max-w-md -translate-x-1/2 -translate-y-1/2 px-4"
          >
            <div className="rounded-2xl border border-white/10 bg-donna-surface p-5 shadow-2xl">
              <h2 id="reset-knowledge-title" className="text-base font-semibold text-white">
                Reset all knowledge?
              </h2>
              <p id="reset-knowledge-desc" className="mt-2 text-sm leading-relaxed text-gray-300">
                This permanently deletes every node and connection on Donna&apos;s mind map.
                She&apos;ll start learning about you again from scratch. This cannot be undone.
              </p>
              <div className="mt-5 flex justify-end gap-2">
                <button
                  type="button"
                  disabled={resetting}
                  onClick={() => setShowResetConfirm(false)}
                  className="rounded-lg border border-white/15 px-4 py-2 text-xs text-gray-200 hover:bg-white/5 disabled:opacity-40"
                >
                  Cancel
                </button>
                <button
                  type="button"
                  disabled={resetting}
                  onClick={resetKnowledge}
                  className="flex items-center gap-2 rounded-lg bg-red-600 px-4 py-2 text-xs font-medium text-white hover:bg-red-500 disabled:opacity-40"
                >
                  {resetting ? <Spinner /> : <Trash2 size={14} />}
                  Reset everything
                </button>
              </div>
            </div>
          </div>
        </>
      )}

      {editing !== null && (
        <NodeEditor
          node={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            load();
          }}
        />
      )}
    </div>
  );
}

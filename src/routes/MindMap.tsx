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

function colorFor(group: string): string {
  if (GROUP_COLORS[group]) return GROUP_COLORS[group];
  let hash = 0;
  for (let i = 0; i < group.length; i++) hash = group.charCodeAt(i) + ((hash << 5) - hash);
  return `hsl(${Math.abs(hash) % 360} 55% 55%)`;
}

function nodeSize(id: string, edges: KgEdge[]): number {
  const links = connectionCount(id, edges);
  return 10 + Math.min(links, 10) * 1.8;
}

function centerToPosition(cx: number, cy: number, size: number) {
  return { x: cx - size / 2, y: cy - size / 2 };
}

function buildFlowNodes(
  graph: KgGraph,
  resolvedEdges: KgEdge[],
  positions: Map<string, { x: number; y: number }>
): Node[] {
  return graph.nodes.map((m) => {
    const size = nodeSize(m.id, resolvedEdges);
    const pos = positions.get(m.id) ?? { x: 0, y: 0 };
    return {
      id: m.id,
      type: "kgCircle",
      position: centerToPosition(pos.x, pos.y, size),
      initialWidth: size,
      initialHeight: size,
      width: size,
      height: size,
      data: {
        label: m.label,
        color: colorFor(m.group || "Topics"),
        size,
      } satisfies KgCircleNodeData,
      draggable: true,
      selectable: true,
    };
  });
}

function applySimPositions(nodes: Node[], sim: ForceSim): Node[] {
  const positions = sim.positions();
  return nodes.map((n) => {
    const size = (n.data as KgCircleNodeData).size;
    const pos = positions.get(n.id) ?? { x: 0, y: 0 };
    return { ...n, position: centerToPosition(pos.x, pos.y, size) };
  });
}

export default function MindMap() {
  const [graph, setGraph] = useState<KgGraph>({ nodes: [], edges: [] });
  const [loading, setLoading] = useState(true);
  const [resetting, setResetting] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
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

  const onNodeDragStart: OnNodeDrag = () => {
    didDragRef.current = false;
  };

  const onNodeDrag: OnNodeDrag = (_e, node) => {
    didDragRef.current = true;
    const sim = simRef.current;
    if (!sim) return;

    const size = (node.data as KgCircleNodeData).size;
    const cx = node.position.x + size / 2;
    const cy = node.position.y + size / 2;
    for (let i = 0; i < 5; i++) {
      sim.tick({ id: node.id, x: cx, y: cy });
    }

    setRfNodes((nds) => applySimPositions(nds, sim));
  };

  const onNodeDragStop: OnNodeDrag = (_e, node) => {
    const sim = simRef.current;
    if (!sim) return;

    const size = (node.data as KgCircleNodeData).size;
    const cx = node.position.x + size / 2;
    const cy = node.position.y + size / 2;
    for (let i = 0; i < 12; i++) {
      sim.tick({ id: node.id, x: cx, y: cy }, 0.6);
    }
    setRfNodes((nds) => applySimPositions(nds, sim));
  };

  const onNodeClick: NodeMouseHandler = (_e, node) => {
    if (didDragRef.current) return;
    const found = nodeById.get(node.id);
    if (found) setEditing(found);
  };

  const onPaneClick = () => setEditing(null);

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
            nodes · drag to rearrange · click for details
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

      {groupsPresent.length > 0 && (
        <div className="absolute bottom-4 right-4 z-10 flex flex-col gap-2 rounded-xl border border-white/10 bg-donna-surface/90 p-3 backdrop-blur">
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
            nodeDragThreshold={6}
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

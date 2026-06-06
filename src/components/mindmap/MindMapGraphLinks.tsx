import { createPortal } from "react-dom";
import { useNodes, useStore } from "@xyflow/react";
import type { KgEdge } from "../../lib/api";
import type { KgCircleNodeData } from "./KgCircleNode";

interface MindMapGraphLinksProps {
  edges: KgEdge[];
  colorForNode: (id: string) => string;
}

function nodeCenter(node: ReturnType<typeof useNodes>[number]) {
  const data = node.data as KgCircleNodeData;
  const w = node.measured?.width ?? node.width ?? data.size ?? 12;
  const h = node.measured?.height ?? node.height ?? data.size ?? 12;
  return {
    x: node.position.x + w / 2,
    y: node.position.y + h / 2,
  };
}

function nodeRadius(node: ReturnType<typeof useNodes>[number]) {
  const data = node.data as KgCircleNodeData;
  const w = node.measured?.width ?? node.width ?? data.size ?? 12;
  return w / 2;
}

/** Stop lines at the circle edge (+ glow padding) so they never pierce nodes. */
function trimLineEndpoints(
  from: { x: number; y: number },
  to: { x: number; y: number },
  r1: number,
  r2: number
) {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const len = Math.hypot(dx, dy);
  if (len < 1) {
    return { x1: from.x, y1: from.y, x2: to.x, y2: to.y, len: 0 };
  }
  const ux = dx / len;
  const uy = dy / len;
  const pad1 = r1 + Math.max(6, r1 * 0.35);
  const pad2 = r2 + Math.max(6, r2 * 0.35);
  return {
    x1: from.x + ux * pad1,
    y1: from.y + uy * pad1,
    x2: to.x - ux * pad2,
    y2: to.y - uy * pad2,
    len,
  };
}

/** Gentle arc bows the edge away from the straight chord to reduce node crossings. */
function curvedPath(x1: number, y1: number, x2: number, y2: number, bow: number): string {
  const mx = (x1 + x2) / 2;
  const my = (y1 + y2) / 2;
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy) || 1;
  const cx = mx + (-dy / len) * bow;
  const cy = my + (dx / len) * bow;
  return `M ${x1} ${y1} Q ${cx} ${cy} ${x2} ${y2}`;
}

function edgeBow(edge: KgEdge, len: number): number {
  let hash = 0;
  const key = `${edge.source}-${edge.target}`;
  for (let i = 0; i < key.length; i++) {
    hash = key.charCodeAt(i) + ((hash << 5) - hash);
  }
  const sign = hash % 2 === 0 ? 1 : -1;
  return sign * Math.min(36, Math.max(12, len * 0.12));
}

/** Draw edges in React Flow's native edges layer (behind nodes). */
export function MindMapGraphLinks({ edges, colorForNode }: MindMapGraphLinksProps) {
  const nodes = useNodes();
  const domNode = useStore((s) => s.domNode);
  const edgesLayer = domNode?.querySelector(".react-flow__edges") ?? null;

  if (!edgesLayer || edges.length === 0 || nodes.length === 0) return null;

  const nodeMap = new Map(nodes.map((n) => [n.id, n]));

  return createPortal(
    <svg
      className="mindmap-links"
      aria-hidden
      style={{
        overflow: "visible",
        position: "absolute",
        pointerEvents: "none",
        left: 0,
        top: 0,
        width: 1,
        height: 1,
      }}
    >
      <g className="mindmap-links__lines">
        {edges.map((edge, i) => {
          const source = nodeMap.get(edge.source);
          const target = nodeMap.get(edge.target);
          if (!source || !target) return null;

          const from = nodeCenter(source);
          const to = nodeCenter(target);
          const { x1, y1, x2, y2, len } = trimLineEndpoints(
            from,
            to,
            nodeRadius(source),
            nodeRadius(target)
          );

          const d = curvedPath(x1, y1, x2, y2, edgeBow(edge, len));
          const color = colorForNode(edge.source);

          return (
            <path
              key={`${edge.source}-${edge.target}-${i}`}
              d={d}
              fill="none"
              stroke={color}
              strokeWidth={2.5}
              strokeOpacity={0.9}
              strokeLinecap="round"
            />
          );
        })}
      </g>
    </svg>,
    edgesLayer
  );
}

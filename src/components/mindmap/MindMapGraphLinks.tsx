import { useMemo } from "react";
import { createPortal } from "react-dom";
import { useNodes, useStore } from "@xyflow/react";
import type { KgEdge } from "../../lib/api";
import { type KgCircleNodeData, CARD_W, CARD_H, PILL_W, PILL_H } from "./KgCircleNode";

interface MindMapGraphLinksProps {
  edges: KgEdge[];
  colorForNode: (id: string) => string;
  colorForGroup: (group: string) => string;
}

function nodeCenter(node: ReturnType<typeof useNodes>[number]) {
  const data = node.data as KgCircleNodeData;
  const defaultW = data.isFolder ? PILL_W : CARD_W;
  const defaultH = data.isFolder ? PILL_H : CARD_H;
  const w = node.measured?.width ?? node.width ?? defaultW;
  const h = node.measured?.height ?? node.height ?? defaultH;
  return {
    x: node.position.x + w / 2,
    y: node.position.y + h / 2,
  };
}

export function MindMapGraphLinks({
  edges,
  colorForNode,
  colorForGroup,
}: MindMapGraphLinksProps) {
  const nodes = useNodes();
  const domNode = useStore((s) => s.domNode);
  const edgesLayer = domNode?.querySelector(".react-flow__edges") ?? null;

  const clusterBlobs = useMemo(() => {
    const groups = new Map<string, { xs: number[]; ys: number[] }>();
    for (const node of nodes) {
      const data = node.data as KgCircleNodeData;
      const group = data.group;
      if (!group) continue;
      const w = node.measured?.width ?? node.width ?? (data.isFolder ? PILL_W : CARD_W);
      const h = node.measured?.height ?? node.height ?? (data.isFolder ? PILL_H : CARD_H);
      const cx = node.position.x + w / 2;
      const cy = node.position.y + h / 2;
      if (!groups.has(group)) groups.set(group, { xs: [], ys: [] });
      groups.get(group)!.xs.push(cx);
      groups.get(group)!.ys.push(cy);
    }

    return [...groups.entries()].map(([group, { xs, ys }]) => {
      const cx = xs.reduce((a, b) => a + b, 0) / xs.length;
      const cy = ys.reduce((a, b) => a + b, 0) / ys.length;
      const maxRx = Math.max(...xs.map((x) => Math.abs(x - cx)), 50);
      const maxRy = Math.max(...ys.map((y) => Math.abs(y - cy)), 40);
      return {
        group,
        cx,
        cy,
        rx: maxRx + 110,
        ry: maxRy + 90,
        color: colorForGroup(group),
      };
    });
  }, [nodes, colorForGroup]);

  if (!edgesLayer || nodes.length === 0) return null;

  const nodeMap = new Map(nodes.map((n) => [n.id, n]));

  return createPortal(
    <svg
      className="mindmap-svg-layer"
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
      <defs>
        <filter
          id="cluster-halo"
          x="-60%"
          y="-60%"
          width="220%"
          height="220%"
        >
          <feGaussianBlur stdDeviation="50" />
        </filter>
      </defs>

      {/* Cluster halos — rendered first (behind edges and nodes) */}
      <g className="cluster-halos">
        {clusterBlobs.map(({ group, cx, cy, rx, ry, color }) => (
          <ellipse
            key={group}
            cx={cx}
            cy={cy}
            rx={rx}
            ry={ry}
            fill={color}
            fillOpacity={0.07}
            filter="url(#cluster-halo)"
          />
        ))}
      </g>

      {/* Edges — bezier curves */}
      <g className="mindmap-edges">
        {edges.map((edge, i) => {
          const source = nodeMap.get(edge.source);
          const target = nodeMap.get(edge.target);
          if (!source || !target) return null;

          const from = nodeCenter(source);
          const to = nodeCenter(target);
          const dx = to.x - from.x;
          const cpx1 = from.x + dx * 0.35;
          const cpx2 = from.x + dx * 0.65;
          const d = `M ${from.x} ${from.y} C ${cpx1} ${from.y} ${cpx2} ${to.y} ${to.x} ${to.y}`;
          const color = colorForNode(edge.source);

          return (
            <path
              key={`${edge.source}-${edge.target}-${i}`}
              d={d}
              stroke={color}
              strokeWidth={1.5}
              strokeOpacity={0.5}
              fill="none"
              strokeLinecap="round"
            />
          );
        })}
      </g>
    </svg>,
    edgesLayer
  );
}

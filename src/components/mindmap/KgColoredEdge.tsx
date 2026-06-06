import { memo } from "react";
import { BaseEdge, getStraightPath, type EdgeProps } from "@xyflow/react";

type KgColoredEdgeData = {
  sourceColor: string;
  targetColor: string;
};

function safeGradientId(id: string): string {
  return `kg-edge-${id.replace(/[^a-zA-Z0-9_-]/g, "_")}`;
}

function KgColoredEdgeComponent({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  data,
}: EdgeProps) {
  const d = (data ?? {}) as KgColoredEdgeData;
  const sourceColor = d.sourceColor ?? "#e8a55a";
  const targetColor = d.targetColor ?? sourceColor;
  const [path] = getStraightPath({ sourceX, sourceY, targetX, targetY });
  const gradientId = safeGradientId(id);

  return (
    <g className="kg-colored-edge">
      <defs>
        <linearGradient
          id={gradientId}
          gradientUnits="userSpaceOnUse"
          x1={sourceX}
          y1={sourceY}
          x2={targetX}
          y2={targetY}
        >
          <stop offset="0%" stopColor={sourceColor} />
          <stop offset="100%" stopColor={targetColor} />
        </linearGradient>
      </defs>
      <BaseEdge
        id={id}
        path={path}
        style={{
          stroke: `url(#${gradientId})`,
          strokeWidth: 2.5,
          strokeOpacity: 1,
          strokeLinecap: "round",
        }}
      />
    </g>
  );
}

export const KgColoredEdge = memo(KgColoredEdgeComponent);

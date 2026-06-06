import { memo } from "react";
import { BaseEdge, getStraightPath, type EdgeProps } from "@xyflow/react";

type KgColoredEdgeData = {
  sourceColor: string;
  targetColor: string;
};

function KgColoredEdgeComponent({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  data,
}: EdgeProps) {
  const d = (data ?? {}) as KgColoredEdgeData;
  const sourceColor = d.sourceColor ?? "#ffffff40";
  const targetColor = d.targetColor ?? sourceColor;
  const [path] = getStraightPath({ sourceX, sourceY, targetX, targetY });
  const gradientId = `kg-edge-${id}`;

  return (
    <>
      <defs>
        <linearGradient
          id={gradientId}
          gradientUnits="userSpaceOnUse"
          x1={sourceX}
          y1={sourceY}
          x2={targetX}
          y2={targetY}
        >
          <stop offset="0%" stopColor={sourceColor} stopOpacity={0.75} />
          <stop offset="100%" stopColor={targetColor} stopOpacity={0.75} />
        </linearGradient>
      </defs>
      <BaseEdge
        id={id}
        path={path}
        style={{
          stroke: `url(#${gradientId})`,
          strokeWidth: 1.5,
          strokeLinecap: "round",
        }}
      />
    </>
  );
}

export const KgColoredEdge = memo(KgColoredEdgeComponent);

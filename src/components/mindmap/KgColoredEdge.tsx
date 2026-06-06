import { BaseEdge, getStraightPath, type EdgeProps } from "@xyflow/react";

type KgColoredEdgeData = {
  sourceColor: string;
  targetColor: string;
};

export function KgColoredEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  data,
}: EdgeProps) {
  const d = (data ?? {}) as KgColoredEdgeData;
  const sourceColor = d.sourceColor ?? "#e8a55a";
  const [path] = getStraightPath({ sourceX, sourceY, targetX, targetY });

  // Solid stroke stays visible while nodes move; gradient url() refs are fragile in SVG.
  const stroke = sourceColor;

  return (
    <BaseEdge
      id={id}
      path={path}
      style={{
        stroke,
        strokeWidth: 2.5,
        strokeOpacity: 0.9,
        strokeLinecap: "round",
      }}
    />
  );
}

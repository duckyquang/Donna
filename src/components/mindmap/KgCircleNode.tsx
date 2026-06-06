import { memo } from "react";
import type { NodeProps } from "@xyflow/react";

export type KgCircleNodeData = {
  label: string;
  color: string;
  size: number;
  isFolder?: boolean;
};

function nodeGlow(color: string, selected: boolean, strong = false) {
  const spread = strong ? 1.25 : 1;
  if (selected) {
    return `0 0 0 2px #fff, 0 0 ${12 * spread}px ${color}ee, 0 0 ${24 * spread}px ${color}bb, 0 0 ${40 * spread}px ${color}66`;
  }
  return `0 0 ${8 * spread}px ${color}dd, 0 0 ${16 * spread}px ${color}99, 0 0 ${28 * spread}px ${color}55`;
}

function KgCircleNodeComponent({ data, selected }: NodeProps) {
  const d = data as KgCircleNodeData;
  const s = d.isFolder ? Math.max(d.size, 22) : d.size;
  const circleClass = d.isFolder
    ? `kg-node-circle kg-node-circle--folder${selected ? " kg-node-circle--selected" : ""}`
    : `kg-node-circle${selected ? " kg-node-circle--selected" : ""}`;

  return (
    <div
      className={d.isFolder ? "kg-node kg-node--folder" : "kg-node"}
      style={{ width: s, height: s }}
      title={d.label}
    >
      <div
        className={circleClass}
        style={{
          width: s,
          height: s,
          backgroundColor: d.color,
          borderColor: "rgba(255, 255, 255, 0.28)",
          boxShadow: nodeGlow(d.color, !!selected, d.isFolder),
        }}
      />
      <span className={`kg-node-label${d.isFolder ? " kg-node-label--folder" : ""}`}>
        {d.label}
      </span>
    </div>
  );
}

export const KgCircleNode = memo(KgCircleNodeComponent);

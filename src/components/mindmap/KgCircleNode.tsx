import { memo } from "react";
import type { NodeProps } from "@xyflow/react";

export type KgCircleNodeData = {
  label: string;
  color: string;
  size: number;
  isFolder?: boolean;
};

function KgCircleNodeComponent({ data, selected }: NodeProps) {
  const d = data as KgCircleNodeData;

  if (d.isFolder) {
    const w = Math.max(d.size * 1.6, 36);
    const h = Math.max(d.size * 0.9, 22);
    return (
      <div className="kg-node kg-node--folder" style={{ width: w, height: h }} title={d.label}>
        <div
          className={`kg-node-folder${selected ? " kg-node-folder--selected" : ""}`}
          style={{
            width: w,
            height: h,
            borderColor: `${d.color}99`,
            backgroundColor: `${d.color}22`,
            boxShadow: selected ? `0 0 0 2px #fff, 0 0 12px ${d.color}66` : undefined,
          }}
        />
        <span className="kg-node-label kg-node-label--folder">{d.label}</span>
      </div>
    );
  }

  return (
    <div className="kg-node" style={{ width: d.size, height: d.size }} title={d.label}>
      <div
        className={`kg-node-circle${selected ? " kg-node-circle--selected" : ""}`}
        style={{
          width: d.size,
          height: d.size,
          backgroundColor: d.color,
          boxShadow: selected ? `0 0 0 2px #fff, 0 0 14px ${d.color}88` : undefined,
        }}
      />
      <span className="kg-node-label">{d.label}</span>
    </div>
  );
}

export const KgCircleNode = memo(KgCircleNodeComponent);

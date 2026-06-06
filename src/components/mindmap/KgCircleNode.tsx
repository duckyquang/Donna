import { memo } from "react";
import type { NodeProps } from "@xyflow/react";

export type KgCircleNodeData = {
  label: string;
  color: string;
  size: number;
};

function KgCircleNodeComponent({ data, selected }: NodeProps) {
  const d = data as KgCircleNodeData;

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

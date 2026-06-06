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
    const s = Math.max(d.size, 22);
    const glow = selected
      ? `0 0 0 2px #fff, 0 0 14px ${d.color}cc, 0 0 28px ${d.color}88, 0 0 42px ${d.color}44`
      : `0 0 10px ${d.color}aa, 0 0 22px ${d.color}66, 0 0 36px ${d.color}33`;
    return (
      <div className="kg-node kg-node--folder" style={{ width: s, height: s }} title={d.label}>
        <div
          className={`kg-node-folder-circle${selected ? " kg-node-folder-circle--selected" : ""}`}
          style={{
            width: s,
            height: s,
            borderColor: d.color,
            backgroundColor: `${d.color}40`,
            boxShadow: glow,
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

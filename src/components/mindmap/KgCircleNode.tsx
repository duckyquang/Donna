import { memo } from "react";
import type { NodeProps } from "@xyflow/react";

export type KgCircleNodeData = {
  label: string;
  color: string;
  note: string;
  nodeType: string;
  group: string;
  isFolder?: boolean;
};

export const CARD_W = 80;
export const CARD_H = 46;
export const PILL_W = 100;
export const PILL_H = 26;

function KgCircleNodeComponent({ data, selected }: NodeProps) {
  const d = data as KgCircleNodeData;

  if (d.isFolder) {
    return (
      <div
        className={`kg-dot-pill${selected ? " kg-dot-pill--selected" : ""}`}
        style={{
          borderColor: selected ? d.color : `${d.color}55`,
          boxShadow: selected ? `0 0 10px ${d.color}40` : "none",
        }}
        title={d.label}
      >
        <span className="kg-dot-pill__dot" style={{ background: d.color }} />
        <span className="kg-dot-pill__label">{d.label}</span>
      </div>
    );
  }

  return (
    <div
      className={`kg-dot-node${selected ? " kg-dot-node--selected" : ""}`}
      title={d.label}
    >
      <div
        className="kg-dot-node__circle"
        style={{
          background: d.color,
          boxShadow: selected
            ? `0 0 0 3px ${d.color}40, 0 0 16px ${d.color}70`
            : `0 0 6px ${d.color}55`,
          transform: selected ? "scale(1.45)" : "scale(1)",
        }}
      />
      <span className="kg-dot-node__label">{d.label}</span>
    </div>
  );
}

export const KgCircleNode = memo(KgCircleNodeComponent);

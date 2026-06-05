import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

export type KgCircleNodeData = {
  label: string;
  color: string;
  size: number;
};

function KgCircleNodeComponent({ data, selected }: NodeProps) {
  const d = data as KgCircleNodeData;

  return (
    <div className="kg-node" title={d.label}>
      <Handle
        type="target"
        position={Position.Top}
        className="kg-node-handle"
        isConnectable={false}
      />
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
      <Handle
        type="source"
        position={Position.Bottom}
        className="kg-node-handle"
        isConnectable={false}
      />
    </div>
  );
}

export const KgCircleNode = memo(KgCircleNodeComponent);

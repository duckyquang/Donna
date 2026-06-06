import { memo, type CSSProperties } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

export type KgCircleNodeData = {
  label: string;
  color: string;
  size: number;
};

function KgCircleNodeComponent({ data, selected }: NodeProps) {
  const d = data as KgCircleNodeData;
  const handleStyle: CSSProperties = {
    opacity: 0,
    width: 1,
    height: 1,
    minWidth: 0,
    minHeight: 0,
    border: "none",
    background: "transparent",
    top: "50%",
    left: "50%",
    transform: "translate(-50%, -50%)",
  };

  return (
    <div className="kg-node" style={{ width: d.size, height: d.size }} title={d.label}>
      <Handle
        type="target"
        position={Position.Top}
        id="target"
        className="kg-node-handle"
        style={handleStyle}
        isConnectable={false}
      />
      <Handle
        type="source"
        position={Position.Top}
        id="source"
        className="kg-node-handle"
        style={handleStyle}
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
    </div>
  );
}

export const KgCircleNode = memo(KgCircleNodeComponent);

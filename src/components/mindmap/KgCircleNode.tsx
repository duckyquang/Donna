import { memo, useEffect } from "react";
import { Handle, Position, useNodeId, useUpdateNodeInternals, type NodeProps } from "@xyflow/react";

export type KgCircleNodeData = {
  label: string;
  color: string;
  size: number;
};

const centerHandleStyle: React.CSSProperties = {
  opacity: 0,
  width: 1,
  height: 1,
  left: "50%",
  top: "50%",
  transform: "translate(-50%, -50%)",
  border: "none",
  background: "transparent",
  minWidth: 0,
  minHeight: 0,
};

function KgCircleNodeComponent({ data, selected }: NodeProps) {
  const d = data as KgCircleNodeData;
  const nodeId = useNodeId();
  const updateNodeInternals = useUpdateNodeInternals();

  useEffect(() => {
    if (nodeId) updateNodeInternals(nodeId);
  }, [nodeId, updateNodeInternals, d.size]);

  return (
    <div className="kg-node" style={{ width: d.size, height: d.size }} title={d.label}>
      <Handle
        id="center-out"
        type="source"
        position={Position.Top}
        style={centerHandleStyle}
        isConnectable={false}
      />
      <Handle
        id="center-in"
        type="target"
        position={Position.Top}
        style={centerHandleStyle}
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

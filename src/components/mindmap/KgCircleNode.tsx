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

export const CARD_W = 168;
export const CARD_H = 68;
export const PILL_W = 116;
export const PILL_H = 30;

function KgCircleNodeComponent({ data, selected }: NodeProps) {
  const d = data as KgCircleNodeData;

  if (d.isFolder) {
    return (
      <div
        className={`kg-folder-pill${selected ? " kg-folder-pill--selected" : ""}`}
        style={{
          borderColor: selected ? d.color : `${d.color}80`,
          boxShadow: selected
            ? `0 0 14px ${d.color}55, 0 0 0 1px ${d.color}40`
            : `0 0 6px ${d.color}22`,
        }}
        title={d.label}
      >
        <span
          className="kg-folder-pill__dot"
          style={{ background: d.color }}
        />
        <span className="kg-folder-pill__label">{d.label}</span>
      </div>
    );
  }

  const notePreview = d.note
    ? d.note.replace(/[*#_`>]/g, "").trim().slice(0, 110)
    : "";

  return (
    <div
      className={`kg-card-node${selected ? " kg-card-node--selected" : ""}`}
      style={{
        borderColor: selected ? `${d.color}cc` : `${d.color}30`,
        boxShadow: selected
          ? `0 0 0 1px ${d.color}60, 0 0 20px ${d.color}30, 0 4px 16px rgba(0,0,0,0.5)`
          : `0 2px 10px rgba(0,0,0,0.35)`,
      }}
      title={d.label}
    >
      <div className="kg-card-stripe" style={{ background: d.color }} />
      <div className="kg-card-body">
        <div className="kg-card-header">
          <span className="kg-card-label">{d.label}</span>
          {d.nodeType && d.nodeType !== "info" && (
            <span className="kg-card-type" style={{ color: d.color }}>
              {d.nodeType}
            </span>
          )}
        </div>
        {notePreview ? (
          <p className="kg-card-note">{notePreview}</p>
        ) : (
          <p className="kg-card-note kg-card-note--empty">No description yet</p>
        )}
      </div>
    </div>
  );
}

export const KgCircleNode = memo(KgCircleNodeComponent);

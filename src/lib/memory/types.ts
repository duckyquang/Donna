export type EntityKind = "person" | "project" | "topic" | "organization";

export interface Entity {
  id: string;
  kind: EntityKind;
  name: string;
}

export type MemorySource = "chat" | "correction" | "integration";

/** A typed fact or preference, optionally linked to an entity, with provenance. */
export interface Memory {
  id: string;
  key: string;
  value: string;
  entityId?: string;
  source: MemorySource;
  createdAt: string; // ISO timestamp
}

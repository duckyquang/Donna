import type { Entity, Memory } from "./types";

export * from "./types";

/**
 * Client for Donna's personal knowledge graph. Persistence lives in the Rust core
 * (SQLite); these methods will call Tauri commands.
 *
 * NOTE: Phase-0 stub. Phase 1 wires structured storage; Phase 4 adds embedding recall.
 */
export const memory = {
  async listEntities(): Promise<Entity[]> {
    return [];
  },
  async listMemories(): Promise<Memory[]> {
    return [];
  },
  async remember(_input: Omit<Memory, "id" | "createdAt">): Promise<void> {
    // TODO(phase-1): invoke Tauri command to persist a memory.
  },
};

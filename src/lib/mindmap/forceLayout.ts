import type { KgEdge } from "../api";

export interface LayoutPoint {
  x: number;
  y: number;
}

type SimNode = { x: number; y: number; vx: number; vy: number };

const LINK_DISTANCE = 100;
const REPULSION = 4200;
const CENTER_PULL = 0.03;
const DAMPING = 0.78;

/** Obsidian-style force-directed layout — nodes spread naturally, edges pull linked pairs together. */
export function forceLayout(
  nodes: { id: string }[],
  edges: KgEdge[],
  iterations = 180
): Map<string, LayoutPoint> {
  if (nodes.length === 0) return new Map();

  const sim = ForceSim.create(nodes.map((n) => n.id), edges);
  const coolingSteps = Math.max(iterations, 1);
  for (let iter = 0; iter < coolingSteps; iter++) {
    const cooling = 1 - iter / coolingSteps;
    sim.tick(undefined, cooling);
  }
  return sim.positions();
}

export function connectionCount(id: string, edges: KgEdge[]): number {
  return edges.filter((e) => e.source === id || e.target === id).length;
}

/** Live force simulation for interactive dragging. */
export class ForceSim {
  private sim = new Map<string, SimNode>();
  private nodeIds: string[];
  private links: KgEdge[];

  private constructor(nodeIds: string[], links: KgEdge[]) {
    this.nodeIds = nodeIds;
    this.links = links.filter(
      (e) => nodeIds.includes(e.source) && nodeIds.includes(e.target)
    );
    for (const id of nodeIds) {
      this.sim.set(id, { x: 0, y: 0, vx: 0, vy: 0 });
    }
  }

  static create(nodeIds: string[], links: KgEdge[]): ForceSim {
    const inst = new ForceSim(nodeIds, links);
    nodeIds.forEach((id, i) => {
      let hash = 0;
      for (let c = 0; c < id.length; c++) {
        hash = id.charCodeAt(c) + ((hash << 5) - hash);
      }
      const angle = (2 * Math.PI * i) / Math.max(nodeIds.length, 1) + (hash % 360) * 0.002;
      const radius = 160 + (Math.abs(hash) % 80);
      const n = inst.sim.get(id)!;
      n.x = Math.cos(angle) * radius;
      n.y = Math.sin(angle) * radius;
    });
    return inst;
  }

  static fromLayout(
    nodeIds: string[],
    links: KgEdge[],
    layout: Map<string, LayoutPoint>
  ): ForceSim {
    const inst = new ForceSim(nodeIds, links);
    for (const id of nodeIds) {
      const p = layout.get(id) ?? { x: 0, y: 0 };
      const n = inst.sim.get(id)!;
      n.x = p.x;
      n.y = p.y;
      n.vx = 0;
      n.vy = 0;
    }
    return inst;
  }

  /** Advance physics one step. Pin a node while dragging so linked nodes follow. */
  tick(pinned?: { id: string; x: number; y: number }, cooling = 1) {
    if (pinned) {
      const p = this.sim.get(pinned.id);
      if (p) {
        p.x = pinned.x;
        p.y = pinned.y;
        p.vx = 0;
        p.vy = 0;
      }
    }

    const scaled = this.scaledForces(cooling);
    scaled(this.nodeIds, this.sim, this.links, pinned?.id);
  }

  private scaledForces(cooling: number) {
    return (
      nodeIds: string[],
      sim: Map<string, SimNode>,
      links: KgEdge[],
      pinnedId?: string
    ) => {
      for (let i = 0; i < nodeIds.length; i++) {
        for (let j = i + 1; j < nodeIds.length; j++) {
          const idA = nodeIds[i]!;
          const idB = nodeIds[j]!;
          const a = sim.get(idA)!;
          const b = sim.get(idB)!;
          let dx = a.x - b.x;
          let dy = a.y - b.y;
          let distSq = dx * dx + dy * dy;
          if (distSq < 1) distSq = 1;
          const force = (REPULSION / distSq) * cooling;
          const dist = Math.sqrt(distSq);
          dx /= dist;
          dy /= dist;
          if (idA !== pinnedId) {
            a.vx += dx * force;
            a.vy += dy * force;
          }
          if (idB !== pinnedId) {
            b.vx -= dx * force;
            b.vy -= dy * force;
          }
        }
      }

      for (const link of links) {
        const a = sim.get(link.source);
        const b = sim.get(link.target);
        if (!a || !b) continue;
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const force = ((dist - LINK_DISTANCE) / dist) * 0.12 * cooling;
        if (link.source !== pinnedId) {
          a.vx += dx * force;
          a.vy += dy * force;
        }
        if (link.target !== pinnedId) {
          b.vx -= dx * force;
          b.vy -= dy * force;
        }
      }

      for (const id of nodeIds) {
        if (id === pinnedId) continue;
        const n = sim.get(id)!;
        n.vx -= n.x * CENTER_PULL * cooling;
        n.vy -= n.y * CENTER_PULL * cooling;
        n.vx *= DAMPING;
        n.vy *= DAMPING;
        n.x += n.vx;
        n.y += n.vy;
      }
    };
  }

  positions(): Map<string, LayoutPoint> {
    const out = new Map<string, LayoutPoint>();
    for (const [id, n] of this.sim) {
      out.set(id, { x: n.x, y: n.y });
    }
    return out;
  }
}

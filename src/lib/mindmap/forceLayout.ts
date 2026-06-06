import type { KgEdge } from "../api";

export interface LayoutPoint {
  x: number;
  y: number;
}

export interface LayoutNode {
  id: string;
  /** Top-level category — clusters stay separated on the canvas. */
  group: string;
  /** Visual radius including glow padding. */
  radius: number;
}

type SimNode = { x: number; y: number; vx: number; vy: number };

const LINK_DISTANCE = 110;
const INTRA_REPULSION = 4800;
const INTER_REPULSION = 32000;
const INTER_GROUP_MIN = 140;
const EDGE_NODE_CLEARANCE = 14;
const EDGE_NODE_REPULSION = 12000;
const CROSS_GROUP_EDGE_REPULSION = 22000;
const CENTER_PULL = 0.018;
const DAMPING = 0.78;

function distPointToSegment(
  px: number,
  py: number,
  x1: number,
  y1: number,
  x2: number,
  y2: number
): { dist: number; cx: number; cy: number } {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const lenSq = dx * dx + dy * dy;
  if (lenSq < 1) {
    return { dist: Math.hypot(px - x1, py - y1), cx: x1, cy: y1 };
  }
  let t = ((px - x1) * dx + (py - y1) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  const cx = x1 + t * dx;
  const cy = y1 + t * dy;
  return { dist: Math.hypot(px - cx, py - cy), cx, cy };
}

function groupCenters(groups: string[]): Map<string, LayoutPoint> {
  const centers = new Map<string, LayoutPoint>();
  const n = groups.length;
  const ring = 340;
  groups.forEach((g, i) => {
    const angle = (2 * Math.PI * i) / Math.max(n, 1) - Math.PI / 2;
    centers.set(g, { x: Math.cos(angle) * ring, y: Math.sin(angle) * ring });
  });
  return centers;
}

function hashJitter(id: string, scale: number): number {
  let hash = 0;
  for (let c = 0; c < id.length; c++) {
    hash = id.charCodeAt(c) + ((hash << 5) - hash);
  }
  return ((hash % 1000) / 1000 - 0.5) * scale;
}

/** Group-clustered force layout — separates categories and keeps edges off unrelated nodes. */
export function forceLayout(
  nodes: LayoutNode[],
  edges: KgEdge[],
  iterations = 260
): Map<string, LayoutPoint> {
  if (nodes.length === 0) return new Map();

  const sim = ForceSim.create(nodes, edges);
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
  private meta = new Map<string, LayoutNode>();
  private nodeIds: string[];
  private links: KgEdge[];

  private constructor(nodes: LayoutNode[], links: KgEdge[]) {
    this.nodeIds = nodes.map((n) => n.id);
    for (const n of nodes) {
      this.meta.set(n.id, n);
      this.sim.set(n.id, { x: 0, y: 0, vx: 0, vy: 0 });
    }
    this.links = links.filter(
      (e) => this.nodeIds.includes(e.source) && this.nodeIds.includes(e.target)
    );
  }

  static create(nodes: LayoutNode[], links: KgEdge[]): ForceSim {
    const inst = new ForceSim(nodes, links);
    const groups = [...new Set(nodes.map((n) => n.group))].sort();
    const centers = groupCenters(groups);

    for (const n of nodes) {
      const c = centers.get(n.group) ?? { x: 0, y: 0 };
      const sn = inst.sim.get(n.id)!;
      sn.x = c.x + hashJitter(n.id, 90);
      sn.y = c.y + hashJitter(n.id + "y", 90);
    }
    return inst;
  }

  static fromLayout(
    nodes: LayoutNode[],
    links: KgEdge[],
    layout: Map<string, LayoutPoint>
  ): ForceSim {
    const inst = new ForceSim(nodes, links);
    for (const n of nodes) {
      const p = layout.get(n.id) ?? { x: 0, y: 0 };
      const sn = inst.sim.get(n.id)!;
      sn.x = p.x;
      sn.y = p.y;
      sn.vx = 0;
      sn.vy = 0;
    }
    return inst;
  }

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
    this.applyForces(cooling, pinned?.id);
  }

  private applyForces(cooling: number, pinnedId?: string) {
    const { nodeIds, sim, links, meta } = this;

    for (let i = 0; i < nodeIds.length; i++) {
      for (let j = i + 1; j < nodeIds.length; j++) {
        const idA = nodeIds[i]!;
        const idB = nodeIds[j]!;
        const a = sim.get(idA)!;
        const b = sim.get(idB)!;
        const gA = meta.get(idA)!.group;
        const gB = meta.get(idB)!.group;
        const sameGroup = gA === gB;

        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let distSq = dx * dx + dy * dy;
        if (distSq < 1) distSq = 1;
        const dist = Math.sqrt(distSq);
        dx /= dist;
        dy /= dist;

        const rA = meta.get(idA)!.radius;
        const rB = meta.get(idB)!.radius;
        const minDist = rA + rB + (sameGroup ? 12 : INTER_GROUP_MIN);

        let force = (sameGroup ? INTRA_REPULSION : INTER_REPULSION) / distSq;
        if (!sameGroup && dist < minDist) {
          force += ((minDist - dist) / minDist) * 2.5 * INTER_REPULSION;
        }
        force *= cooling;

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
      const force = ((dist - LINK_DISTANCE) / dist) * 0.1 * cooling;
      if (link.source !== pinnedId) {
        a.vx += dx * force;
        a.vy += dy * force;
      }
      if (link.target !== pinnedId) {
        b.vx -= dx * force;
        b.vy -= dy * force;
      }
    }

    for (const link of links) {
      const src = sim.get(link.source);
      const tgt = sim.get(link.target);
      if (!src || !tgt) continue;
      const linkGroup = meta.get(link.source)!.group;

      for (const id of nodeIds) {
        if (id === link.source || id === link.target) continue;
        const n = sim.get(id)!;
        const m = meta.get(id)!;
        const { dist, cx, cy } = distPointToSegment(
          n.x,
          n.y,
          src.x,
          src.y,
          tgt.x,
          tgt.y
        );
        const clearance = m.radius + EDGE_NODE_CLEARANCE;
        if (dist >= clearance) continue;

        let nx = n.x - cx;
        let ny = n.y - cy;
        const nlen = Math.hypot(nx, ny);
        if (nlen < 0.5) {
          nx = n.y - src.y;
          ny = -(n.x - src.x);
        } else {
          nx /= nlen;
          ny /= nlen;
        }

        const crossGroup = m.group !== linkGroup;
        const push =
          ((clearance - dist) / clearance) *
          (crossGroup ? CROSS_GROUP_EDGE_REPULSION : EDGE_NODE_REPULSION) *
          cooling;

        if (id !== pinnedId) {
          n.vx += nx * push;
          n.vy += ny * push;
        }
      }
    }

    for (const id of nodeIds) {
      if (id === pinnedId) continue;
      const n = sim.get(id)!;
      const g = meta.get(id)!.group;
      const groups = [...new Set([...meta.values()].map((m) => m.group))];
      const centers = groupCenters(groups);
      const c = centers.get(g) ?? { x: 0, y: 0 };
      n.vx += (c.x - n.x) * CENTER_PULL * 0.35 * cooling;
      n.vy += (c.y - n.y) * CENTER_PULL * 0.35 * cooling;
      n.vx -= n.x * CENTER_PULL * 0.15 * cooling;
      n.vy -= n.y * CENTER_PULL * 0.15 * cooling;
      n.vx *= DAMPING;
      n.vy *= DAMPING;
      n.x += n.vx;
      n.y += n.vy;
    }
  }

  positions(): Map<string, LayoutPoint> {
    const out = new Map<string, LayoutPoint>();
    for (const [id, n] of this.sim) {
      out.set(id, { x: n.x, y: n.y });
    }
    return out;
  }
}

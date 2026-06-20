import type { KgEdge } from "../api";

export interface LayoutPoint {
  x: number;
  y: number;
}

type SimNode = { x: number; y: number; vx: number; vy: number };

const LINK_DISTANCE = 140;
const REPULSION = 6000;
const CLUSTER_PULL = 0.07;
const CENTER_PULL = 0.006;
const DAMPING = 0.82;
const CLUSTER_RADIUS = 380;

export function computeClusterTargets(groups: string[]): Map<string, LayoutPoint> {
  const unique = [...new Set(groups)].filter(Boolean).sort();
  if (unique.length === 0) return new Map();
  return new Map(
    unique.map((g, i) => [
      g,
      {
        x: Math.cos((2 * Math.PI * i) / unique.length) * CLUSTER_RADIUS,
        y: Math.sin((2 * Math.PI * i) / unique.length) * CLUSTER_RADIUS,
      },
    ])
  );
}

/** Force-directed layout with cluster grouping support. Nodes in the same group
 *  are attracted toward a shared centroid; different groups push each other apart. */
export function forceLayout(
  nodes: { id: string; group?: string }[],
  edges: KgEdge[],
  iterations = 280
): Map<string, LayoutPoint> {
  if (nodes.length === 0) return new Map();

  const groupOf = new Map(nodes.map((n) => [n.id, n.group ?? ""]));
  const groups = [...new Set(nodes.map((n) => n.group ?? ""))].filter(Boolean);
  const clusterTargets = computeClusterTargets(groups);

  const sim = ForceSim.create(
    nodes.map((n) => n.id),
    edges,
    groupOf,
    clusterTargets
  );

  for (let iter = 0; iter < iterations; iter++) {
    const cooling = 1 - iter / iterations;
    sim.tick(undefined, cooling);
  }

  return sim.positions();
}

export function connectionCount(id: string, edges: KgEdge[]): number {
  return edges.filter((e) => e.source === id || e.target === id).length;
}

export class ForceSim {
  private sim = new Map<string, SimNode>();
  private nodeIds: string[];
  private links: KgEdge[];
  private groupOf: Map<string, string>;
  private clusterTargets: Map<string, LayoutPoint>;

  private constructor(
    nodeIds: string[],
    links: KgEdge[],
    groupOf: Map<string, string>,
    clusterTargets: Map<string, LayoutPoint>
  ) {
    this.nodeIds = nodeIds;
    this.links = links.filter(
      (e) => nodeIds.includes(e.source) && nodeIds.includes(e.target)
    );
    this.groupOf = groupOf;
    this.clusterTargets = clusterTargets;
    for (const id of nodeIds) {
      this.sim.set(id, { x: 0, y: 0, vx: 0, vy: 0 });
    }
  }

  static create(
    nodeIds: string[],
    links: KgEdge[],
    groupOf: Map<string, string> = new Map(),
    clusterTargets: Map<string, LayoutPoint> = new Map()
  ): ForceSim {
    const inst = new ForceSim(nodeIds, links, groupOf, clusterTargets);
    nodeIds.forEach((id, i) => {
      const group = groupOf.get(id) ?? "";
      const centroid = clusterTargets.get(group);
      const baseX = centroid?.x ?? 0;
      const baseY = centroid?.y ?? 0;

      let hash = 0;
      for (let c = 0; c < id.length; c++) {
        hash = id.charCodeAt(c) + ((hash << 5) - hash);
      }
      const jitter = 50 + (Math.abs(hash) % 40);
      const angle =
        (2 * Math.PI * i) / Math.max(nodeIds.length, 1) + (hash % 100) * 0.063;
      const n = inst.sim.get(id)!;
      n.x = baseX + Math.cos(angle) * jitter;
      n.y = baseY + Math.sin(angle) * jitter;
    });
    return inst;
  }

  static fromLayout(
    nodeIds: string[],
    links: KgEdge[],
    layout: Map<string, LayoutPoint>,
    groupOf: Map<string, string> = new Map(),
    clusterTargets: Map<string, LayoutPoint> = new Map()
  ): ForceSim {
    const inst = new ForceSim(nodeIds, links, groupOf, clusterTargets);
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
    const { nodeIds, sim, links, groupOf, clusterTargets } = this;

    // Repulsion (stronger between different clusters)
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
        const dist = Math.sqrt(distSq);
        const sameGroup =
          (groupOf.get(idA) ?? "") === (groupOf.get(idB) ?? "");
        const rep = (REPULSION * (sameGroup ? 1 : 1.8)) / distSq * cooling;
        dx /= dist;
        dy /= dist;
        if (idA !== pinnedId) { a.vx += dx * rep; a.vy += dy * rep; }
        if (idB !== pinnedId) { b.vx -= dx * rep; b.vy -= dy * rep; }
      }
    }

    // Link attraction
    for (const link of links) {
      const a = sim.get(link.source);
      const b = sim.get(link.target);
      if (!a || !b) continue;
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const force = ((dist - LINK_DISTANCE) / dist) * 0.11 * cooling;
      if (link.source !== pinnedId) { a.vx += dx * force; a.vy += dy * force; }
      if (link.target !== pinnedId) { b.vx -= dx * force; b.vy -= dy * force; }
    }

    // Cluster pull toward group centroid
    for (const id of nodeIds) {
      if (id === pinnedId) continue;
      const n = sim.get(id)!;
      const group = groupOf.get(id) ?? "";
      const centroid = clusterTargets.get(group);
      if (centroid) {
        n.vx += (centroid.x - n.x) * CLUSTER_PULL * cooling;
        n.vy += (centroid.y - n.y) * CLUSTER_PULL * cooling;
      }
    }

    // Weak center pull + damping + integrate
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
  }

  positions(): Map<string, LayoutPoint> {
    const out = new Map<string, LayoutPoint>();
    for (const [id, n] of this.sim) {
      out.set(id, { x: n.x, y: n.y });
    }
    return out;
  }
}

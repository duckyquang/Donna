import type { KgEdge, KgNode } from "../api";

export interface LayoutPoint {
  x: number;
  y: number;
}

/** Obsidian-style force-directed layout — nodes spread naturally, edges pull linked pairs together. */
export function forceLayout(
  nodes: KgNode[],
  edges: KgEdge[],
  iterations = 180
): Map<string, LayoutPoint> {
  if (nodes.length === 0) return new Map();

  const ids = new Set(nodes.map((n) => n.id));
  const links = edges.filter((e) => ids.has(e.source) && ids.has(e.target));

  type SimNode = { x: number; y: number; vx: number; vy: number };
  const sim = new Map<string, SimNode>();

  nodes.forEach((n, i) => {
    let hash = 0;
    for (let c = 0; c < n.id.length; c++) {
      hash = n.id.charCodeAt(c) + ((hash << 5) - hash);
    }
    const angle = (2 * Math.PI * i) / nodes.length + (hash % 360) * 0.002;
    const radius = 160 + (Math.abs(hash) % 80);
    sim.set(n.id, {
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
      vx: 0,
      vy: 0,
    });
  });

  const linkDistance = 100;
  const repulsion = 4200;
  const centerPull = 0.04;
  const damping = 0.82;

  for (let iter = 0; iter < iterations; iter++) {
    const cooling = 1 - iter / iterations;

    // Repulsion between all nodes.
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const a = sim.get(nodes[i].id)!;
        const b = sim.get(nodes[j].id)!;
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let distSq = dx * dx + dy * dy;
        if (distSq < 1) distSq = 1;
        const force = (repulsion / distSq) * cooling;
        const dist = Math.sqrt(distSq);
        dx /= dist;
        dy /= dist;
        a.vx += dx * force;
        a.vy += dy * force;
        b.vx -= dx * force;
        b.vy -= dy * force;
      }
    }

    // Attraction along edges.
    for (const link of links) {
      const a = sim.get(link.source)!;
      const b = sim.get(link.target)!;
      let dx = b.x - a.x;
      let dy = b.y - a.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const force = ((dist - linkDistance) / dist) * 0.08 * cooling;
      a.vx += dx * force;
      a.vy += dy * force;
      b.vx -= dx * force;
      b.vy -= dy * force;
    }

    // Gentle pull toward center.
    for (const n of sim.values()) {
      n.vx -= n.x * centerPull * cooling;
      n.vy -= n.y * centerPull * cooling;
      n.vx *= damping;
      n.vy *= damping;
      n.x += n.vx;
      n.y += n.vy;
    }
  }

  const out = new Map<string, LayoutPoint>();
  for (const [id, n] of sim) {
    out.set(id, { x: n.x, y: n.y });
  }
  return out;
}

export function connectionCount(id: string, edges: KgEdge[]): number {
  return edges.filter((e) => e.source === id || e.target === id).length;
}

// Axial hex to screen, pointy-top. Same convention as core/src/hex.rs.
// The whole grid is turned by ANGLE so the island sits like the printed
// board, whose hex grid runs 15 degrees off the board's edges.

import type { Hex } from "./types";

export const SIZE = 26; // hex outer radius in px
const ANGLE = (-15 * Math.PI) / 180;
const COS = Math.cos(ANGLE);
const SIN = Math.sin(ANGLE);

function rot(x: number, y: number): { x: number; y: number } {
  return { x: x * COS - y * SIN, y: x * SIN + y * COS };
}

export function center(h: Hex): { x: number; y: number } {
  return rot(SIZE * Math.sqrt(3) * (h.q + h.r / 2), SIZE * 1.5 * h.r);
}

export function corners(cx: number, cy: number, size = SIZE): string {
  const pts: string[] = [];
  for (let i = 0; i < 6; i++) {
    const a = ANGLE + (Math.PI / 180) * (60 * i - 30);
    pts.push(`${cx + size * Math.cos(a)},${cy + size * Math.sin(a)}`);
  }
  return pts.join(" ");
}

export type Point = { x: number; y: number };

/// Corner `i` of a hex: 0 is upper right, then clockwise. Matches `corners`.
export function corner(cx: number, cy: number, i: number, size = SIZE): Point {
  const a = ANGLE + (Math.PI / 180) * (60 * i - 30);
  return { x: cx + size * Math.cos(a), y: cy + size * Math.sin(a) };
}

/// The six neighbour offsets, in the order of core/src/hex.rs: E, NE, NW, W, SW, SE.
export const DIRECTIONS: Hex[] = [
  { q: 1, r: 0 },
  { q: 1, r: -1 },
  { q: 0, r: -1 },
  { q: -1, r: 0 },
  { q: -1, r: 1 },
  { q: 0, r: 1 },
];

// The two corners on the side facing each direction.
const EDGE_CORNERS = [
  [0, 1],
  [5, 0],
  [4, 5],
  [3, 4],
  [2, 3],
  [1, 2],
];

export function neighbor(h: Hex, d: number): Hex {
  return { q: h.q + DIRECTIONS[d].q, r: h.r + DIRECTIONS[d].r };
}

/// The side of `h` it shares with its neighbour in direction `d`.
export function edge(h: Hex, d: number): [Point, Point] {
  const c = center(h);
  const [a, b] = EDGE_CORNERS[d];
  return [corner(c.x, c.y, a), corner(c.x, c.y, b)];
}

export function key(h: Hex): string {
  return `${h.q},${h.r}`;
}

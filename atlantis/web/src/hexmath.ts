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

export function key(h: Hex): string {
  return `${h.q},${h.r}`;
}

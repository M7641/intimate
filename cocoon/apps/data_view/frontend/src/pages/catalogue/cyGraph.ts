// Cytoscape glue for the catalogue map: theme palettes, stylesheet, and the
// translation from our Catalogue response into graph elements. Kept apart from
// the page component so the Solid wiring stays readable.

import type { ElementDefinition, StylesheetJson } from "cytoscape";
import type { Catalogue, CatalogueEdge } from "@/pages/_shared/types";

export type ThemeName = "light" | "dark" | "cyber";

export interface GraphPalette {
  nodeFill: string;
  nodeBorder: string;
  nodeText: string;
  declared: string; // high-confidence FK edges
  inferred: string; // naming-convention guesses
  highlight: string; // the selected edge + its endpoints
}

// Canvas-safe colours (no oklch) picked to sit on each theme's background.
export const PALETTES: Record<ThemeName, GraphPalette> = {
  light: {
    nodeFill: "#ffffff",
    nodeBorder: "#d4d4d4",
    nodeText: "#404040",
    declared: "#7c3aed",
    inferred: "#9ca3af",
    highlight: "#2563eb",
  },
  dark: {
    nodeFill: "#262626",
    nodeBorder: "#52525b",
    nodeText: "#fafafa",
    declared: "#a78bfa",
    inferred: "#71717a",
    highlight: "#60a5fa",
  },
  cyber: {
    nodeFill: "#241a47",
    nodeBorder: "#4b3b8f",
    nodeText: "#e9d5ff",
    declared: "#f472d0",
    inferred: "#6d28d9",
    highlight: "#22d3ee",
  },
};

/** Short human label for an edge: the shared column, or `a→b` when names differ. */
export function edgeLabel(edge: CatalogueEdge): string {
  return edge.links
    .map((l) =>
      l.source_column === l.target_column
        ? l.source_column
        : `${l.source_column}→${l.target_column}`,
    )
    .join(", ");
}

/** Stable, collision-free id for an edge. */
export function edgeId(edge: CatalogueEdge): string {
  return `${edge.source}__${edge.target}__${edge.kind}`;
}

/** Approximate pixel width to fit a node's label (Cytoscape no longer auto-sizes). */
function nodeWidth(label: string): number {
  return Math.max(48, label.length * 8 + 24);
}

/** Translate the catalogue response into Cytoscape elements. */
export function toElements(catalogue: Catalogue): ElementDefinition[] {
  const nodes: ElementDefinition[] = catalogue.nodes.map((n) => ({
    data: {
      id: n.table_name,
      label: n.table_name,
      columnCount: n.column_count,
      w: nodeWidth(n.table_name),
    },
  }));

  // Cytoscape throws if an edge references a node it doesn't have, so drop any
  // edge whose endpoint isn't in the node set (e.g. a declared FK to a table
  // outside this schema).
  const ids = new Set(catalogue.nodes.map((n) => n.table_name));
  const edges: ElementDefinition[] = catalogue.edges
    .filter((e) => ids.has(e.source) && ids.has(e.target))
    .map((e) => ({
      data: {
        id: edgeId(e),
        source: e.source,
        target: e.target,
        kind: e.kind,
        label: edgeLabel(e),
        edge: e, // carry the full edge so the tap handler can read its links
      },
    }));

  return [...nodes, ...edges];
}

/** Build the themed stylesheet. Re-applied whenever the theme changes. */
export function buildStylesheet(p: GraphPalette): StylesheetJson {
  return [
    {
      selector: "node",
      style: {
        "background-color": p.nodeFill,
        "border-color": p.nodeBorder,
        "border-width": 1,
        shape: "round-rectangle",
        label: "data(label)",
        color: p.nodeText,
        "font-size": 12,
        "font-family": "Geist Variable, sans-serif",
        "text-valign": "center",
        "text-halign": "center",
        width: "data(w)",
        height: 30,
      },
    },
    {
      selector: "edge",
      style: {
        width: 1.5,
        "line-color": p.inferred,
        "line-style": "dashed",
        "curve-style": "bezier",
        label: "data(label)",
        "font-size": 9,
        color: p.inferred,
        "text-rotation": "autorotate",
        "text-background-color": p.nodeFill,
        "text-background-opacity": 0.85,
        "text-background-padding": "2px",
      },
    },
    {
      // Declared FKs are ground truth: solid line + a direction arrow.
      selector: 'edge[kind = "declared"]',
      style: {
        "line-color": p.declared,
        color: p.declared,
        "line-style": "solid",
        width: 2,
        "target-arrow-color": p.declared,
        "target-arrow-shape": "triangle",
      },
    },
    {
      selector: "edge.hl",
      style: {
        "line-color": p.highlight,
        color: p.highlight,
        "target-arrow-color": p.highlight,
        width: 3,
        "font-size": 11,
        "z-index": 10,
      },
    },
    {
      selector: "node.hl",
      style: {
        "border-color": p.highlight,
        "border-width": 2,
      },
    },
  ];
}

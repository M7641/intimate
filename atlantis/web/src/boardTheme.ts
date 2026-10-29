// How the board looks. Every number here has a visible effect: change one,
// save, and the dev page reloads with it. Nothing here changes the rules.
//
// Distances are in board pixels: a hex is about 45 px across.

/** Patches of a second colour, laid over a material before the light. */
export type Mottle = {
  color: string;
  /** Patch frequency, per pixel. Higher is smaller patches. */
  grain: number;
  /** 0 is no patches, 1 covers most of the ground. */
  amount: number;
};

/** A procedural texture: noise, lit from one side, tinted with a colour. */
export type Material = {
  /** Base colour. The light only darkens it. */
  base: string;
  /** Noise frequency, per pixel. Higher is finer grain. */
  grain: number;
  /** Noise octaves, 1 to 6. More adds finer detail on top of the grain. */
  detail: number;
  /** 0 is flat colour, 1 is full light and shade. */
  contrast: number;
  /** Height of the bumps the light falls on. */
  relief: number;
  /** Noise pattern. Change it for a different layout of the same texture. */
  seed: number;
  mottle?: Mottle;
};

export const MATERIALS = {
  sand: {
    base: "#f4dfb2", grain: 0.03, detail: 4, contrast: 0.3, relief: 9, seed: 1,
    mottle: { color: "#fbecc9", grain: 0.012, amount: 0.45 },
  },
  dirt: {
    base: "#d9a676", grain: 0.04, detail: 4, contrast: 0.38, relief: 12, seed: 2,
    mottle: { color: "#c48f5f", grain: 0.014, amount: 0.5 },
  },
  grass: {
    base: "#8fc251", grain: 0.22, detail: 2, contrast: 0.45, relief: 4, seed: 3,
    mottle: { color: "#4c8a2a", grain: 0.06, amount: 0.6 },
  },
  rock: {
    base: "#9a948c", grain: 0.05, detail: 5, contrast: 0.6, relief: 16, seed: 4,
    mottle: { color: "#6e6963", grain: 0.03, amount: 0.45 },
  },
  water: {
    base: "#4fb2df", grain: 0.012, detail: 3, contrast: 0.25, relief: 7, seed: 5,
    mottle: { color: "#3b9fd3", grain: 0.008, amount: 0.5 },
  },
} satisfies Record<string, Material>;

export type MaterialName = keyof typeof MATERIALS;

/** Ground material per zone letter (see data/board.ron). */
export const ZONE_MATERIAL: Record<string, MaterialName> = {
  A: "dirt", // Red Throne
  B: "sand", // Red Outpost
  C: "dirt", // Battlefield
  D: "sand", // Blue Outpost
  E: "dirt", // Blue Throne
  F: "grass", // North Jungle
  G: "grass", // South Jungle
};

/** Terrain spaces, and the sea around the island. */
export const TERRAIN_MATERIAL: MaterialName = "rock";
export const SEA_MATERIAL: MaterialName = "water";

/** Where the light comes from, in degrees. */
export const LIGHT = { azimuth: 225, elevation: 60 };

/**
 * Bends the island's outline and inner borders so they stop following hex
 * sides. The hex grid itself stays straight. Strength 0 turns it off.
 */
export const WARP = { grain: 0.022, strength: 22, seed: 7 };

/** Rounds the hex corners off the island's outline. 0 keeps them sharp. */
export const COAST_ROUNDING = 5;

/** The island's shadow on the water. */
export const ISLAND_SHADOW = { dx: 3, dy: 6, blur: 5, color: "#0b3a5c", opacity: 0.45 };

/** Rocks stand above the ground and shade the spaces next to them. */
export const ROCK_SHADOW = { dx: 2, dy: 3, blur: 2.5, color: "#2a1d10", opacity: 0.55 };

/** Rounds the hex corners off rocks, so they read as boulders. 0 keeps them sharp. */
export const ROCK_ROUNDING = 6;

/** Bare earth under every land space. It shows where rounded rocks pull back. */
export const SOIL = "#7d6248";

/** Light water along the coast, and the foam line where it meets land. */
export const SHORE = {
  shallows: "#a6e6ef",
  shallowsWidth: 26,
  shallowsBlur: 7,
  shallowsOpacity: 0.75,
  foam: "#ffffff",
  foamWidth: 4,
  foamOpacity: 0.8,
};

/**
 * The crack between two zones. A light line just below the dark one makes
 * it read as cut into the ground.
 */
export const BORDER = {
  color: "#5b3a1e",
  opacity: 0.6,
  width: 1.8,
  highlight: "#fff3d6",
  highlightOpacity: 0.45,
};

/** The hex grid drawn over the ground, with bright points at its corners. */
export const GRID = { color: "#fff8e6", opacity: 0.28, width: 0.8, vertex: 1.4, vertexOpacity: 0.55 };

/** The outline around the current battle zone. */
export const BATTLE = { color: "#ffd75e", width: 3, glow: 3, opacity: 0.9 };

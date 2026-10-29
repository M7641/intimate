// The painted board under the clickable hexes: sea, shore, textured ground,
// zone borders, grid corners, battle zone outline. Takes no clicks.
//
// Every look setting comes from boardTheme.ts.

import { For, Show, createMemo, untrack } from "solid-js";
import type { SpaceView, Zone } from "./types";
import { SIZE, center, corner, corners, edge, key, neighbor, type Point } from "./hexmath";
import {
  BATTLE,
  BORDER,
  COAST_ROUNDING,
  GRID,
  ISLAND_SHADOW,
  LIGHT,
  MATERIALS,
  ROCK_ROUNDING,
  ROCK_SHADOW,
  SEA_MATERIAL,
  SHORE,
  SOIL,
  TERRAIN_MATERIAL,
  WARP,
  ZONE_MATERIAL,
  type Material,
  type MaterialName,
} from "./boardTheme";

export type Box = { x: number; y: number; w: number; h: number };

type Props = {
  spaces: SpaceView[];
  zones: Map<number, Zone>;
  box: Box;
  battleZone: number;
};

function materialOf(s: SpaceView, z: Zone): MaterialName {
  if (z.kind.type === "Sea") return SEA_MATERIAL;
  if (s.terrain) return TERRAIN_MATERIAL;
  return ZONE_MATERIAL[z.letter] ?? "sand";
}

function path(edges: [Point, Point][]): string {
  return edges.map(([a, b]) => `M${a.x} ${a.y}L${b.x} ${b.y}`).join("");
}

/// Everything about the board that does not change during a game.
function layout(spaces: SpaceView[], zones: Map<number, Zone>) {
  const at = new Map<string, SpaceView>();
  for (const s of spaces) at.set(key(s.hex), s);
  const sea = (s: SpaceView | undefined) => !s || zones.get(s.zone)!.kind.type === "Sea";

  const byMaterial = new Map<MaterialName, SpaceView[]>();
  const shore: [Point, Point][] = [];
  const cracks: [Point, Point][] = [];
  const vertices = new Map<string, Point>();
  for (const s of spaces) {
    if (sea(s)) continue;
    const name = materialOf(s, zones.get(s.zone)!);
    if (!byMaterial.has(name)) byMaterial.set(name, []);
    byMaterial.get(name)!.push(s);
    for (let d = 0; d < 6; d++) {
      const n = at.get(key(neighbor(s.hex, d)));
      if (sea(n)) {
        shore.push(edge(s.hex, d));
      } else if (d < 3 && !s.terrain && !n!.terrain && n!.zone !== s.zone) {
        // Directions 0-2 meet every pair of neighbours once.
        cracks.push(edge(s.hex, d));
      }
    }
    if (!s.terrain) {
      const c = center(s.hex);
      for (let i = 0; i < 6; i++) {
        const p = corner(c.x, c.y, i);
        vertices.set(`${Math.round(p.x)},${Math.round(p.y)}`, p);
      }
    }
  }
  // Rock last, so its shadow falls on the ground around it.
  const groups = [...byMaterial.entries()].sort(
    ([a], [b]) => Number(a === TERRAIN_MATERIAL) - Number(b === TERRAIN_MATERIAL),
  );
  return { at, groups, shore: path(shore), cracks: path(cracks), vertices: [...vertices.values()] };
}

/// A signature of the board's shape, so a new snapshot of the same board
/// does not repaint the ground.
function signature(spaces: SpaceView[]): string {
  return spaces.map((s) => `${s.hex.q},${s.hex.r}:${s.zone}${s.terrain ? "#" : "."}`).join(" ");
}

/// Noise, lit from one side, tinted, cut to the shape it is applied to.
function TextureFilter(props: { name: string; m: Material }) {
  const m = props.m;
  const mottle = m.mottle;
  // Mottle: keep the noise above a threshold, with a soft edge.
  const threshold = mottle ? 0.75 - 0.5 * mottle.amount : 0;
  return (
    <filter id={`tex-${props.name}`} x="0" y="0" width="100%" height="100%" color-interpolation-filters="sRGB">
      <feTurbulence type="fractalNoise" baseFrequency={m.grain} numOctaves={m.detail} seed={m.seed} result="noise" />
      <feDiffuseLighting in="noise" surfaceScale={m.relief} lighting-color="#fff" result="light">
        <feDistantLight azimuth={LIGHT.azimuth} elevation={LIGHT.elevation} />
      </feDiffuseLighting>
      <feComponentTransfer in="light" result="shade">
        <feFuncR type="linear" slope={m.contrast} intercept={1 - m.contrast} />
        <feFuncG type="linear" slope={m.contrast} intercept={1 - m.contrast} />
        <feFuncB type="linear" slope={m.contrast} intercept={1 - m.contrast} />
      </feComponentTransfer>
      <feFlood flood-color={m.base} result="flat" />
      <Show when={mottle} fallback={<feMerge result="base"><feMergeNode in="flat" /></feMerge>}>
        {(mt) => (
          <>
            <feTurbulence type="fractalNoise" baseFrequency={mt().grain} numOctaves={2} seed={m.seed + 100} result="patchNoise" />
            <feColorMatrix in="patchNoise" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  1 0 0 0 0" result="patchRaw" />
            <feComponentTransfer in="patchRaw" result="patchMask">
              <feFuncA type="linear" slope={8} intercept={-8 * threshold} />
            </feComponentTransfer>
            <feFlood flood-color={mt().color} result="patchColor" />
            <feComposite in="patchColor" in2="patchMask" operator="in" result="patches" />
            <feComposite in="patches" in2="flat" operator="over" result="base" />
          </>
        )}
      </Show>
      <feBlend in="base" in2="shade" mode="multiply" result="lit" />
      <feComposite in="lit" in2="SourceAlpha" operator="in" />
    </filter>
  );
}

/// The same warp for every layer that must line up with the ground.
function Warp(props: { result: string }) {
  return (
    <>
      <feTurbulence type="fractalNoise" baseFrequency={WARP.grain} numOctaves={1} seed={WARP.seed} result="warpNoise" />
      <feDisplacementMap
        in="SourceGraphic"
        in2="warpNoise"
        scale={WARP.strength}
        xChannelSelector="R"
        yChannelSelector="G"
        result={props.result}
      />
    </>
  );
}

/// A blurred, tinted copy of `input`'s shape, moved, under `input`.
function Shadow(props: { input: string; s: typeof ISLAND_SHADOW }) {
  return (
    <>
      <feFlood flood-color={props.s.color} flood-opacity={props.s.opacity} result="shadowColor" />
      <feComposite in="shadowColor" in2={props.input} operator="in" result="shadowShape" />
      <feGaussianBlur in="shadowShape" stdDeviation={props.s.blur} result="shadowBlur" />
      <feOffset in="shadowBlur" dx={props.s.dx} dy={props.s.dy} result="shadow" />
      <feMerge>
        <feMergeNode in="shadow" />
        <feMergeNode in={props.input} />
      </feMerge>
    </>
  );
}

export function Ground(props: Props) {
  const shape = createMemo(() => signature(props.spaces));
  const board = createMemo(() => {
    shape();
    return untrack(() => layout(props.spaces, props.zones));
  });

  // The current battle zone's walkable spaces, outlined.
  const battleOutline = createMemo(() => {
    const { at } = board();
    const inside = (s: SpaceView | undefined) => !!s && s.zone === props.battleZone && !s.terrain;
    const edges: [Point, Point][] = [];
    for (const s of at.values()) {
      if (!inside(s)) continue;
      for (let d = 0; d < 6; d++) {
        if (!inside(at.get(key(neighbor(s.hex, d))))) edges.push(edge(s.hex, d));
      }
    }
    return path(edges);
  });

  // Filters work in board pixels over the whole board.
  const region = () => ({
    filterUnits: "userSpaceOnUse" as const,
    x: props.box.x,
    y: props.box.y,
    width: props.box.w,
    height: props.box.h,
  });

  return (
    <g class="ground">
      <defs>
        <For each={Object.entries(MATERIALS)}>{([name, m]) => <TextureFilter name={name} m={m} />}</For>
        <filter id="warp" {...region()}>
          <Warp result="warped" />
        </filter>
        <filter id="warp-island-shadow" {...region()}>
          <Warp result="warped" />
          {/* Blur the outline, then cut it at half height: corners round off. */}
          <feGaussianBlur in="warped" stdDeviation={COAST_ROUNDING} result="soft" />
          <feComponentTransfer in="soft" result="rounded">
            <feFuncA type="linear" slope={8} intercept={-3.6} />
          </feComponentTransfer>
          <feComposite in="warped" in2="rounded" operator="in" result="island" />
          <Shadow input="island" s={ISLAND_SHADOW} />
        </filter>
        <filter id="shallows" {...region()}>
          <Warp result="warped" />
          <feGaussianBlur in="warped" stdDeviation={SHORE.shallowsBlur} />
        </filter>
        <filter id="rock-shadow" {...region()}>
          <feGaussianBlur in="SourceGraphic" stdDeviation={ROCK_ROUNDING} result="soft" />
          <feComponentTransfer in="soft" result="rounded">
            <feFuncA type="linear" slope={8} intercept={-3.6} />
          </feComponentTransfer>
          <feComposite in="SourceGraphic" in2="rounded" operator="in" result="boulders" />
          <Shadow input="boulders" s={ROCK_SHADOW} />
        </filter>
        <filter id="battle-glow" {...region()}>
          <feGaussianBlur in="SourceGraphic" stdDeviation={BATTLE.glow} result="glow" />
          <feMerge>
            <feMergeNode in="glow" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      <rect
        x={props.box.x}
        y={props.box.y}
        width={props.box.w}
        height={props.box.h}
        filter={`url(#tex-${SEA_MATERIAL})`}
      />

      <path
        d={board().shore}
        filter="url(#shallows)"
        fill="none"
        stroke={SHORE.shallows}
        stroke-width={SHORE.shallowsWidth}
        stroke-opacity={SHORE.shallowsOpacity}
        stroke-linecap="round"
      />
      <path
        d={board().shore}
        filter="url(#warp)"
        fill="none"
        stroke={SHORE.foam}
        stroke-width={SHORE.foamWidth}
        stroke-opacity={SHORE.foamOpacity}
        stroke-linecap="round"
      />

      <g filter="url(#warp-island-shadow)">
        <g fill={SOIL}>
          <For each={board().groups}>
            {([, spaces]) => (
              <For each={spaces}>
                {(s) => {
                  const c = center(s.hex);
                  return <polygon points={corners(c.x, c.y, SIZE + 0.8)} />;
                }}
              </For>
            )}
          </For>
        </g>
        <For each={board().groups}>
          {([name, spaces]) => {
            const texture = (
              <g filter={`url(#tex-${name})`}>
                <For each={spaces}>
                  {(s) => {
                    const c = center(s.hex);
                    // A touch larger than the hex, so neighbours leave no seam.
                    return <polygon points={corners(c.x, c.y, SIZE + 0.8)} />;
                  }}
                </For>
              </g>
            );
            return name === TERRAIN_MATERIAL ? <g filter="url(#rock-shadow)">{texture}</g> : texture;
          }}
        </For>
        <path
          d={board().cracks}
          transform="translate(0.9 0.9)"
          fill="none"
          stroke={BORDER.highlight}
          stroke-opacity={BORDER.highlightOpacity}
          stroke-width={BORDER.width}
          stroke-linecap="round"
        />
        <path
          d={board().cracks}
          fill="none"
          stroke={BORDER.color}
          stroke-opacity={BORDER.opacity}
          stroke-width={BORDER.width}
          stroke-linecap="round"
        />
      </g>

      <g fill={GRID.color} fill-opacity={GRID.vertexOpacity}>
        <For each={board().vertices}>{(p) => <circle cx={p.x} cy={p.y} r={GRID.vertex} />}</For>
      </g>

      <path
        d={battleOutline()}
        filter="url(#battle-glow)"
        fill="none"
        stroke={BATTLE.color}
        stroke-opacity={BATTLE.opacity}
        stroke-width={BATTLE.width}
        stroke-linecap="round"
      />
    </g>
  );
}

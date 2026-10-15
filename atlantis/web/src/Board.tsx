import { For, Show, createMemo } from "solid-js";
import type { Hex, SpaceView, Snapshot, Unit, Zone } from "./types";
import { SIZE, center, corners, key } from "./hexmath";

const ZONE_FILL: Record<string, string> = {
  Throne_Red: "#e8b4a8",
  Throne_Blue: "#a8c4e8",
  Lane: "#e6d9b8",
  Side: "#b9d4a6",
  Sea: "#7fb8d8",
};

function zoneFill(z: Zone): string {
  const k = z.kind;
  return ZONE_FILL[k.type === "Throne" ? `Throne_${k.team}` : k.type];
}

export const TEAM_COLOR = { Red: "#c0392b", Blue: "#2c6fbd" };
const MINION_LETTER = { Melee: "M", Ranged: "R", Heavy: "H" };

/// Short label for a unit: minion type letter, or the hero's initials.
export function unitLabel(u: Unit): string {
  if (u.kind.type === "Minion") return MINION_LETTER[u.kind.kind];
  return u.kind.name.split(" ")[0].slice(0, 2).toUpperCase();
}

/// What clicking a hex would do, derived from the legal actions.
export type Mark = "move" | "travel" | "respawn" | "attack";

type Props = {
  snapshot: Snapshot;
  selected: Hex | null;
  marks: Map<string, Mark>;
  activeHero: number | null;
  onSelect: (hex: Hex) => void;
};

export function Board(props: Props) {
  const zones = createMemo(() => {
    const byId = new Map<number, Zone>();
    for (const z of props.snapshot.zones) byId.set(z.id, z);
    return byId;
  });

  const unitsOnBoard = createMemo(() => props.snapshot.units.filter((u) => u.hex !== null));

  // Bounding box of every hex, with a margin of one hex.
  const viewBox = createMemo(() => {
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const s of props.snapshot.spaces) {
      const c = center(s.hex);
      minX = Math.min(minX, c.x);
      maxX = Math.max(maxX, c.x);
      minY = Math.min(minY, c.y);
      maxY = Math.max(maxY, c.y);
    }
    const m = SIZE * 1.2;
    return `${minX - m} ${minY - m} ${maxX - minX + 2 * m} ${maxY - minY + 2 * m}`;
  });

  const isSelected = (s: SpaceView) =>
    props.selected !== null && key(props.selected) === key(s.hex);

  return (
    <svg class="board" viewBox={viewBox()} preserveAspectRatio="xMidYMid meet">
      {/* Layer 1: spaces. These take the clicks. */}
      <For each={props.snapshot.spaces}>
        {(s) => {
          const c = center(s.hex);
          const zone = zones().get(s.zone)!;
          const sea = zone.kind.type === "Sea";
          const mark = () => props.marks.get(key(s.hex));
          return (
            <g
              class="space"
              classList={{
                selected: isSelected(s),
                terrain: s.terrain,
                sea,
                battle: s.zone === props.snapshot.battle_zone,
                marked: mark() !== undefined,
              }}
              onClick={() => props.onSelect(s.hex)}
            >
              <polygon
                points={corners(c.x, c.y)}
                fill={s.terrain && !sea ? "#5b5b5b" : zoneFill(zone)}
              />
              <Show when={s.spawn}>
                {(sp) => {
                  const p = sp();
                  return p.type === "Hero" ? (
                    <polygon
                      class="spawn"
                      points={corners(c.x, c.y, SIZE * 0.5)}
                      stroke={TEAM_COLOR[p.team]}
                    />
                  ) : (
                    <>
                      <circle class="spawn" cx={c.x} cy={c.y} r={SIZE * 0.42} stroke={TEAM_COLOR[p.team]} />
                      <text x={c.x} y={c.y} class="spawn-letter" fill={TEAM_COLOR[p.team]}>
                        {MINION_LETTER[p.kind]}
                      </text>
                    </>
                  );
                }}
              </Show>
              <Show when={mark()}>
                {(m) => (
                  <polygon
                    class={`mark mark-${m()}`}
                    points={corners(c.x, c.y, SIZE * 0.86)}
                  />
                )}
              </Show>
              <Show when={!sea}>
                <text x={c.x} y={c.y + SIZE * 0.72} class="zone-letter">
                  {zone.letter}
                </text>
              </Show>
            </g>
          );
        }}
      </For>
      {/* Layer 2: units. Clicks fall through to the space below. */}
      <g class="units">
        <For each={unitsOnBoard()}>
          {(u) => {
            const c = center(u.hex!);
            const hero = u.kind.type === "Hero";
            return (
              <g class="unit" classList={{ hero, active: props.activeHero === u.id }}>
                <circle cx={c.x} cy={c.y} r={SIZE * (hero ? 0.58 : 0.4)} fill={TEAM_COLOR[u.team]} />
                <text x={c.x} y={c.y} class="unit-letter">
                  {unitLabel(u)}
                </text>
              </g>
            );
          }}
        </For>
      </g>
    </svg>
  );
}

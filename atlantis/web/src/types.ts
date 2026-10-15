// Mirror of the engine's Snapshot and Action (core/src/game.rs, turn.rs).
// Keep in step by hand.

export type Team = "Red" | "Blue";
export type MinionKind = "Melee" | "Ranged" | "Heavy";

export type Hex = { q: number; r: number };

export type ZoneKind =
  | { type: "Throne"; team: Team }
  | { type: "Lane" }
  | { type: "Side" }
  | { type: "Sea" };

export type Zone = {
  id: number;
  letter: string;
  name: string;
  kind: ZoneKind;
  lane_index: number | null;
};

export type SpawnPoint =
  | { type: "Hero"; team: Team }
  | { type: "Minion"; team: Team; kind: MinionKind };

export type SpaceView = {
  hex: Hex;
  zone: number;
  terrain: boolean;
  spawn: SpawnPoint | null;
};

export type UnitKind =
  | { type: "Hero"; name: string }
  | { type: "Minion"; kind: MinionKind };

export type Unit = {
  id: number;
  team: Team;
  kind: UnitKind;
  hex: Hex | null;
};

export type Color = "GOLD" | "SILVER" | "RED" | "GREEN" | "BLUE" | "PURPLE";
export type ActionType = "ATTACK" | "SKILL" | "DEFENSE" | "MOVEMENT" | "DEFENSE_SKILL";
export type ItemIcon = "ATTACK" | "DEFENSE" | "INITIATIVE" | "MOVEMENT" | "RANGE" | "AREA";

export type Card = {
  id: number;
  name: string;
  text: string;
  color: Color;
  tier: number | null;
  variant: { first: number; second: number } | null;
  handicapped: boolean;
  extra: boolean;
  initiative: number;
  primaryAction: ActionType | null;
  primaryValue: number | null;
  primaryValueSign: "PLUS" | "MINUS" | "EXCLAMATION" | null;
  modifier: "RANGE" | "AREA" | null;
  modifierValue: number | null;
  modifierValueSign: "PLUS" | "MINUS" | "EXCLAMATION" | null;
  secondaryAttack: number | null;
  secondaryMovement: number | null;
  secondaryDefense: number | null;
  item: ItemIcon | null;
  traits: string[];
};

// Mirror of the engine's hero sheets (core/src/hero.rs).
export type Rating = { base: number; peak: number | null };

export type HeroProfile = {
  key: string;
  name: string;
  stars: number;
  set: string;
  roles: string[];
  attack: Rating;
  initiative: Rating;
  defense: Rating;
  movement: Rating;
};

export type HeroSheet = { profile: HeroProfile; cards: Card[] };

export type Player = {
  id: number;
  team: Team;
  hero_key: string;
  hero_name: string;
  hero: number;
  level: number;
  coins: number;
  lowest_tier: number;
  hand: Card[];
  played: Card | null;
  resolved: Card[];
  discard: Card[];
  deck: Card[];
  items: [ItemIcon, number][];
  ultimate: Card | null;
};

export type Phase =
  | { type: "Play" }
  | { type: "ChooseActor"; candidates: number[] }
  | { type: "Act"; player: number }
  | { type: "Defend"; attacker: number; defender: number; attack_total: number }
  | { type: "LevelUp"; player: number }
  | { type: "GameOver"; winner: Team; reason: string };

export type Action =
  | { type: "PlayCard"; player: number; card: number }
  | { type: "Pass"; player: number }
  | { type: "ChooseActor"; player: number }
  | { type: "Respawn"; player: number; hex: Hex }
  | { type: "Move"; player: number; to: Hex }
  | { type: "FastTravel"; player: number; to: Hex }
  | { type: "Attack"; player: number; target: number }
  | { type: "Skill"; player: number }
  | { type: "Hold"; player: number }
  | { type: "Defend"; player: number; card: number | null }
  | { type: "Upgrade"; player: number; card_out: number; take: number };

export type Snapshot = {
  zones: Zone[];
  spaces: SpaceView[];
  zone_adjacency: [number, number][];
  units: Unit[];
  battle_zone: number;
  round: number;
  turn: number;
  waves: number;
  lives: { red: number; blue: number };
  tie_breaker: Team;
  phase: Phase;
  players: Player[];
  actions: Action[];
  log: string[];
};

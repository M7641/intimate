//! The board: hex spaces grouped into zones, with terrain and spawn points.
//!
//! A board is parsed from a text layout (see `data/board.ron`). Each row is a
//! list of two-character cells: the zone letter, then a feature:
//!
//! | feature | meaning                                   |
//! |---------|-------------------------------------------|
//! | `.`     | empty space                               |
//! | `#`     | terrain (an obstacle)                     |
//! | `*`     | hero spawn point (team from the throne)   |
//! | `m` `M` | melee minion spawn, red / blue            |
//! | `s` `S` | ranged minion spawn, red / blue           |
//! | `h` `H` | heavy minion spawn, red / blue            |
//! | `~~`    | sea: terrain, in the zone of kind `Sea`   |
//! | `__`    | no hex here (outside the board)           |
//!
//! Rows use odd-r offset layout: odd rows are shoved right by half a hex.

use crate::hex::Hex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Team {
    Red,
    Blue,
}

impl Team {
    pub const fn opponent(self) -> Team {
        match self {
            Team::Red => Team::Blue,
            Team::Blue => Team::Red,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MinionKind {
    Melee,
    Ranged,
    Heavy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SpawnPoint {
    Hero { team: Team },
    Minion { team: Team, kind: MinionKind },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ZoneId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ZoneKind {
    /// A team's home. Heroes start and respawn here. Losing it loses the game.
    Throne { team: Team },
    /// On the lane between the thrones. Minions push through these.
    Lane,
    /// Off the lane. Heroes only.
    Side,
    /// The water around the island. Every space is terrain.
    Sea,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    pub id: ZoneId,
    pub letter: char,
    pub name: String,
    pub kind: ZoneKind,
    /// Position along the lane, red throne first. `None` for side zones.
    pub lane_index: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    pub zone: ZoneId,
    pub terrain: bool,
    pub spawn: Option<SpawnPoint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub zones: Vec<Zone>,
    pub spaces: BTreeMap<Hex, Space>,
}

#[derive(Debug, thiserror::Error)]
pub enum BoardError {
    #[error("board file is not valid RON: {0}")]
    Syntax(#[from] ron::error::SpannedError),
    #[error("row {row}, cell {col}: {msg}")]
    Cell { row: usize, col: usize, msg: String },
    #[error("zone list must start with the red throne and end with the blue throne")]
    LaneEnds,
    #[error("zone '{0}' is declared twice")]
    DuplicateZone(char),
    #[error("row {row}, cell {col}: sea cell but no zone of kind Sea")]
    NoSeaZone { row: usize, col: usize },
}

/// The on-disk shape of a board file.
#[derive(Deserialize)]
struct BoardSpec {
    zones: Vec<ZoneSpec>,
    rows: Vec<String>,
}

#[derive(Deserialize)]
struct ZoneSpec {
    letter: char,
    name: String,
    kind: ZoneKindSpec,
}

/// File-side spelling of [`ZoneKind`]: `Throne(Red)`, `Lane`, `Side`.
#[derive(Clone, Copy, Deserialize)]
enum ZoneKindSpec {
    Throne(Team),
    Lane,
    Side,
    Sea,
}

impl From<ZoneKindSpec> for ZoneKind {
    fn from(spec: ZoneKindSpec) -> ZoneKind {
        match spec {
            ZoneKindSpec::Throne(team) => ZoneKind::Throne { team },
            ZoneKindSpec::Lane => ZoneKind::Lane,
            ZoneKindSpec::Side => ZoneKind::Side,
            ZoneKindSpec::Sea => ZoneKind::Sea,
        }
    }
}

const SINGLE_LANE: &str = include_str!("../../data/board.ron");

impl Board {
    /// The single-lane map shipped with the game.
    pub fn single_lane() -> Board {
        Board::parse(SINGLE_LANE).expect("bundled board is valid")
    }

    pub fn parse(text: &str) -> Result<Board, BoardError> {
        let spec: BoardSpec = ron::from_str(text)?;
        let zones = build_zones(&spec.zones)?;
        let mut spaces = BTreeMap::new();
        for (row, line) in spec.rows.iter().enumerate() {
            for (col, cell) in line.split_whitespace().enumerate() {
                let Some(space) = parse_cell(cell, &zones, row, col)? else {
                    continue;
                };
                spaces.insert(Hex::from_offset(col as i32, row as i32), space);
            }
        }
        Ok(Board { zones, spaces })
    }

    pub fn zone(&self, id: ZoneId) -> &Zone {
        &self.zones[id.0 as usize]
    }

    pub fn space(&self, hex: Hex) -> Option<&Space> {
        self.spaces.get(&hex)
    }

    pub fn spaces_in_zone(&self, id: ZoneId) -> impl Iterator<Item = (Hex, &Space)> {
        self.spaces
            .iter()
            .filter(move |(_, s)| s.zone == id)
            .map(|(h, s)| (*h, s))
    }

    pub fn throne(&self, team: Team) -> &Zone {
        self.zones
            .iter()
            .find(|z| z.kind == ZoneKind::Throne { team })
            .expect("every board has two thrones")
    }

    /// Two zones are adjacent when at least two non-terrain spaces touch
    /// across their border (rulebook, "Adjacent").
    pub fn zones_adjacent(&self, a: ZoneId, b: ZoneId) -> bool {
        if a == b {
            return false;
        }
        let touching = self
            .spaces_in_zone(a)
            .filter(|(_, s)| !s.terrain)
            .flat_map(|(h, _)| h.neighbors())
            .filter(|n| self.space(*n).is_some_and(|s| s.zone == b && !s.terrain))
            .count();
        touching >= 2
    }

    /// Every adjacent zone pair, each listed once.
    pub fn zone_adjacency(&self) -> Vec<(ZoneId, ZoneId)> {
        let mut pairs = Vec::new();
        for a in &self.zones {
            for b in &self.zones {
                if a.id < b.id && self.zones_adjacent(a.id, b.id) {
                    pairs.push((a.id, b.id));
                }
            }
        }
        pairs
    }
}

fn build_zones(specs: &[ZoneSpec]) -> Result<Vec<Zone>, BoardError> {
    let mut zones = Vec::with_capacity(specs.len());
    let mut lane_index = 0u8;
    for (i, spec) in specs.iter().enumerate() {
        if specs[..i].iter().any(|z| z.letter == spec.letter) {
            return Err(BoardError::DuplicateZone(spec.letter));
        }
        let kind = ZoneKind::from(spec.kind);
        let on_lane = matches!(kind, ZoneKind::Throne { .. } | ZoneKind::Lane);
        let index = on_lane.then(|| {
            lane_index += 1;
            lane_index - 1
        });
        zones.push(Zone {
            id: ZoneId(i as u8),
            letter: spec.letter,
            name: spec.name.clone(),
            kind,
            lane_index: index,
        });
    }
    let lane: Vec<&Zone> = zones.iter().filter(|z| z.lane_index.is_some()).collect();
    let red_first = lane.first().map(|z| z.kind) == Some(ZoneKind::Throne { team: Team::Red });
    let blue_last = lane.last().map(|z| z.kind) == Some(ZoneKind::Throne { team: Team::Blue });
    if !(red_first && blue_last) {
        return Err(BoardError::LaneEnds);
    }
    Ok(zones)
}

fn parse_cell(
    cell: &str,
    zones: &[Zone],
    row: usize,
    col: usize,
) -> Result<Option<Space>, BoardError> {
    let err = |msg: String| BoardError::Cell { row, col, msg };
    let mut chars = cell.chars();
    let (Some(letter), Some(feature), None) = (chars.next(), chars.next(), chars.next()) else {
        return Err(err(format!("'{cell}' is not a two-character cell")));
    };
    if letter == '_' {
        return Ok(None);
    }
    if letter == '~' {
        let sea = zones
            .iter()
            .find(|z| z.kind == ZoneKind::Sea)
            .ok_or(BoardError::NoSeaZone { row, col })?;
        return Ok(Some(Space {
            zone: sea.id,
            terrain: true,
            spawn: None,
        }));
    }
    let zone = zones
        .iter()
        .find(|z| z.letter == letter)
        .ok_or_else(|| err(format!("unknown zone '{letter}'")))?;
    let mut space = Space {
        zone: zone.id,
        terrain: false,
        spawn: None,
    };
    let minion = |kind, team| Some(SpawnPoint::Minion { team, kind });
    space.spawn = match feature {
        '.' => None,
        '#' => {
            space.terrain = true;
            None
        }
        '*' => match zone.kind {
            ZoneKind::Throne { team } => Some(SpawnPoint::Hero { team }),
            _ => return Err(err("hero spawn outside a throne".into())),
        },
        'm' => minion(MinionKind::Melee, Team::Red),
        'M' => minion(MinionKind::Melee, Team::Blue),
        's' => minion(MinionKind::Ranged, Team::Red),
        'S' => minion(MinionKind::Ranged, Team::Blue),
        'h' => minion(MinionKind::Heavy, Team::Red),
        'H' => minion(MinionKind::Heavy, Team::Blue),
        other => return Err(err(format!("unknown feature '{other}'"))),
    };
    Ok(Some(space))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_count(board: &Board, zone: ZoneId, team: Team) -> usize {
        board
            .spaces_in_zone(zone)
            .filter(
                |(_, s)| matches!(s.spawn, Some(SpawnPoint::Minion { team: t, .. }) if t == team),
            )
            .count()
    }

    #[test]
    fn bundled_board_parses() {
        let board = Board::single_lane();
        assert!(board.spaces.len() > 50);
        assert_eq!(board.throne(Team::Red).lane_index, Some(0));
        let last = board.zones.iter().filter_map(|z| z.lane_index).max();
        assert_eq!(board.throne(Team::Blue).lane_index, last);
    }

    #[test]
    fn thrones_have_three_hero_spawns() {
        let board = Board::single_lane();
        for team in [Team::Red, Team::Blue] {
            let throne = board.throne(team).id;
            let heroes = board
                .spaces_in_zone(throne)
                .filter(|(_, s)| s.spawn == Some(SpawnPoint::Hero { team }))
                .count();
            assert_eq!(heroes, 3, "{team:?} throne");
        }
    }

    #[test]
    fn battle_zone_spawns_six_minions_per_team() {
        let board = Board::single_lane();
        let middle = board
            .zones
            .iter()
            .find(|z| z.letter == 'C')
            .expect("zone C is the starting battle zone");
        assert_eq!(spawn_count(&board, middle.id, Team::Red), 6);
        assert_eq!(spawn_count(&board, middle.id, Team::Blue), 6);
    }

    #[test]
    fn lane_zones_chain_from_throne_to_throne() {
        let board = Board::single_lane();
        let mut lane: Vec<&Zone> = board
            .zones
            .iter()
            .filter(|z| z.lane_index.is_some())
            .collect();
        lane.sort_by_key(|z| z.lane_index);
        for pair in lane.windows(2) {
            assert!(
                board.zones_adjacent(pair[0].id, pair[1].id),
                "{} and {} must touch",
                pair[0].name,
                pair[1].name
            );
        }
    }

    #[test]
    fn sea_is_terrain_and_touches_no_zone() {
        let board = Board::single_lane();
        let sea = board
            .zones
            .iter()
            .find(|z| z.kind == ZoneKind::Sea)
            .expect("the island has a sea");
        let mut n = 0;
        for (_, s) in board.spaces_in_zone(sea.id) {
            assert!(s.terrain);
            n += 1;
        }
        assert!(n > 20);
        for z in &board.zones {
            assert!(
                !board.zones_adjacent(sea.id, z.id),
                "sea touches {}",
                z.name
            );
        }
        assert_eq!(sea.lane_index, None);
    }

    /// A space written back in board-file notation, `__` for no space.
    fn cell_code(board: &Board, hex: Hex) -> String {
        let Some(space) = board.space(hex) else {
            return "__".into();
        };
        let zone = board.zone(space.zone);
        if zone.kind == ZoneKind::Sea {
            return "~~".into();
        }
        let feature = match space.spawn {
            _ if space.terrain => '#',
            None => '.',
            Some(SpawnPoint::Hero { .. }) => '*',
            Some(SpawnPoint::Minion { team, kind }) => {
                let c = match kind {
                    MinionKind::Melee => 'm',
                    MinionKind::Ranged => 's',
                    MinionKind::Heavy => 'h',
                };
                if team == Team::Blue {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            }
        };
        format!("{}{feature}", zone.letter)
    }

    /// What a cell must read as on the far side of the board: zones swap
    /// ends, spawns swap teams, sea and no-space are both off the island.
    fn twin_code(code: &str) -> String {
        if code == "__" {
            return "~~".into();
        }
        let mut chars = code.chars();
        let zone = match chars.next().unwrap() {
            'A' => 'E',
            'E' => 'A',
            'B' => 'D',
            'D' => 'B',
            'F' => 'G',
            'G' => 'F',
            other => other,
        };
        let f = chars.next().unwrap();
        let feature = if f.is_ascii_uppercase() {
            f.to_ascii_lowercase()
        } else {
            f.to_ascii_uppercase()
        };
        format!("{zone}{feature}")
    }

    #[test]
    fn board_is_point_symmetric() {
        // Turned half a circle, the map reads the same with red and blue
        // swapped. The hero spawns pair up across the centre, so their sum
        // is three times the doubled centre.
        let board = Board::single_lane();
        let heroes: Vec<Hex> = board
            .spaces
            .iter()
            .filter(|(_, s)| matches!(s.spawn, Some(SpawnPoint::Hero { .. })))
            .map(|(h, _)| *h)
            .collect();
        assert_eq!(heroes.len(), 6);
        let sum = heroes.iter().fold(Hex::ZERO, |a, h| a + *h);
        assert!(
            sum.q % 3 == 0 && sum.r % 3 == 0,
            "hero spawns are not paired"
        );
        let double_centre = Hex::new(sum.q / 3, sum.r / 3);

        let mut mismatches = Vec::new();
        for &hex in board.spaces.keys() {
            let twin = double_centre - hex;
            if twin < hex {
                continue;
            }
            let here = cell_code(&board, hex);
            let there = cell_code(&board, twin);
            let off_island = |c: &str| c == "~~" || c == "__";
            if off_island(&here) && off_island(&there) {
                continue;
            }
            if twin_code(&here) != there {
                let (c1, r1) = hex.to_offset();
                let (c2, r2) = twin.to_offset();
                mismatches.push((
                    (r1, c1),
                    format!(
                        "row {r1:2} cell {c1:2}: {here}   vs   row {r2:2} cell {c2:2}: {there}"
                    ),
                ));
            }
        }
        mismatches.sort();
        let mismatches: Vec<String> = mismatches.into_iter().map(|(_, m)| m).collect();
        assert!(
            mismatches.is_empty(),
            "{} asymmetric pairs:\n{}",
            mismatches.len(),
            mismatches.join("\n")
        );
    }

    #[test]
    fn one_touching_space_is_not_adjacency() {
        let text = r#"(
            zones: [
                (letter: 'A', name: "Red Throne", kind: Throne(Red)),
                (letter: 'B', name: "Blue Throne", kind: Throne(Blue)),
            ],
            rows: [
                "A. A. A.",
                "__ B# B.",
            ],
        )"#;
        let board = Board::parse(text).unwrap();
        // Only one non-terrain B space touches A.
        assert!(!board.zones_adjacent(ZoneId(0), ZoneId(1)));
    }

    #[test]
    fn rejects_hero_spawn_outside_throne() {
        let text = r#"(
            zones: [
                (letter: 'A', name: "Red Throne", kind: Throne(Red)),
                (letter: 'L', name: "Lane", kind: Lane),
                (letter: 'B', name: "Blue Throne", kind: Throne(Blue)),
            ],
            rows: ["A. L* B."],
        )"#;
        assert!(matches!(
            Board::parse(text),
            Err(BoardError::Cell { col: 1, .. })
        ));
    }
}

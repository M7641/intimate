//! Game state and its snapshot.
//!
//! [`Game`] owns everything a running match needs. Frontends and AIs read a
//! [`Snapshot`], pick one of its `actions`, and call [`Game::apply`].

use crate::board::{Board, MinionKind, SpawnPoint, Team, Zone, ZoneId, ZoneKind};
use crate::card::{CardDef, CardId, Catalog, Color, ItemIcon};
use crate::hero::hero_profile;
use crate::hex::Hex;
use crate::player::{Player, PlayerId};
use crate::rules::{self, GameLength};
use crate::sim::Rng;
use crate::turn::{Action, Phase};
use crate::unit::{Unit, UnitId, UnitKind};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

/// Who plays. Hero keys per team ("wasp"), in seating order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setup {
    pub red: Vec<String>,
    pub blue: Vec<String>,
    pub length: GameLength,
    /// Drives the tie-breaker flip. Nothing else in the rules is random.
    pub seed: u64,
    /// The team whose heroes take the first player seats. The rules do not
    /// care; it fixes the order players are listed and enumerated in.
    pub seated_first: Team,
}

impl Setup {
    /// The rulebook's first-game line-up: 4 players, quick game.
    pub fn teaching_game() -> Setup {
        let keys = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
        Setup {
            red: keys(&["wasp", "xargatha"]),
            blue: keys(&["arien", "brogan"]),
            length: rules::quick_game(4),
            seed: 1,
            seated_first: Team::Red,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SetupError {
    #[error("{team:?} throne has {available} hero spawn points, {needed} heroes given")]
    TooManyHeroes {
        team: Team,
        needed: usize,
        available: usize,
    },
    #[error("the lane must have an odd number of zones so one sits in the middle")]
    NoMiddleZone,
    #[error("no hero called '{0}' in the catalogue")]
    UnknownHero(String),
}

#[derive(Clone, Debug)]
pub struct Game {
    pub board: Board,
    pub catalog: Rc<Catalog>,
    pub units: BTreeMap<UnitId, Unit>,
    pub players: Vec<Player>,
    /// Where the minions currently fight.
    pub battle_zone: ZoneId,
    pub round: u16,
    pub turn: u8,
    pub waves: u8,
    /// Life counters left, indexed by [`Game::team_index`].
    pub lives: [u8; 2],
    pub tie_breaker: Team,
    pub phase: Phase,
    pub log: Vec<String>,
    /// The tie-breaker coin flips once the current actor resolves.
    pub(crate) flip_after_actor: bool,
    /// Players owed an upgrade choice, one entry per level bought.
    pub(crate) pending_level_ups: VecDeque<PlayerId>,
    next_unit: u32,
}

impl Game {
    pub fn new(setup: &Setup) -> Result<Game, SetupError> {
        Game::with_catalog(setup, Rc::new(Catalog::load()))
    }

    /// Share one parsed catalogue across many games (simulations).
    pub fn with_catalog(setup: &Setup, catalog: Rc<Catalog>) -> Result<Game, SetupError> {
        let board = Board::single_lane();
        let battle_zone = middle_lane_zone(&board)?;
        let mut rng = Rng::new(setup.seed);
        let tie_breaker = if rng.below(2) == 0 {
            Team::Red
        } else {
            Team::Blue
        };
        let mut game = Game {
            board,
            catalog,
            units: BTreeMap::new(),
            players: Vec::new(),
            battle_zone,
            round: 1,
            turn: 1,
            waves: setup.length.waves,
            lives: [setup.length.lives; 2],
            tie_breaker,
            phase: Phase::Play,
            log: Vec::new(),
            flip_after_actor: false,
            pending_level_ups: VecDeque::new(),
            next_unit: 0,
        };
        let first = setup.seated_first;
        let heroes = |t: Team| {
            if t == Team::Red {
                &setup.red
            } else {
                &setup.blue
            }
        };
        game.seat_team(first, heroes(first))?;
        game.seat_team(first.opponent(), heroes(first.opponent()))?;
        game.spawn_battle_zone_minions();
        game.log(format!(
            "Tie breaker shows {tie_breaker:?}. Round 1, turn 1."
        ));
        Ok(game)
    }

    /// The rulebook's first game: 4 players on the single-lane map.
    pub fn teaching_game() -> Game {
        Game::new(&Setup::teaching_game()).expect("bundled setup fits the bundled board")
    }

    fn seat_team(&mut self, team: Team, keys: &[String]) -> Result<(), SetupError> {
        let throne = self.board.throne(team).id;
        let spawns: Vec<Hex> = self
            .board
            .spaces_in_zone(throne)
            .filter(|(_, s)| s.spawn == Some(SpawnPoint::Hero { team }))
            .map(|(h, _)| h)
            .collect();
        // Turned half a circle, the board lists hexes in reverse order. Blue
        // takes its spawn points last to first, so each team's n-th hero
        // starts on the twin of the other's.
        let mut spawns = spawns;
        if team == Team::Blue {
            spawns.reverse();
        }
        if keys.len() > spawns.len() {
            return Err(SetupError::TooManyHeroes {
                team,
                needed: keys.len(),
                available: spawns.len(),
            });
        }
        let catalog = self.catalog.clone();
        for (key, hex) in keys.iter().zip(spawns) {
            let deck = catalog
                .deck(key)
                .ok_or_else(|| SetupError::UnknownHero(key.clone()))?;
            let hero_name = hero_profile(key)
                .map(|p| p.name)
                .unwrap_or_else(|| key.clone());
            let hero = self.add_unit(
                team,
                UnitKind::Hero {
                    name: hero_name.clone(),
                },
                hex,
            );
            // Starting hand: the real Gold, Silver, and the three Tier I cards.
            let starts =
                |c: &CardDef| (c.is_basic() && !c.handicapped && !c.extra) || c.tier == Some(1);
            let hand: Vec<CardId> = deck.iter().filter(|c| starts(c)).map(|c| c.id).collect();
            let deck_rest: Vec<CardId> = deck
                .iter()
                .filter(|c| !starts(c) && !c.handicapped)
                .map(|c| c.id)
                .collect();
            self.players.push(Player {
                id: PlayerId(self.players.len() as u8),
                team,
                hero_key: key.clone(),
                hero_name,
                hero,
                hand,
                played: None,
                resolved: Vec::new(),
                discard: Vec::new(),
                deck: deck_rest,
                item_cards: Vec::new(),
                ultimate: None,
                coins: 0,
                level: 1,
                passed: false,
                leveled_this_round: false,
            });
        }
        Ok(())
    }

    /// One minion per minion spawn point in the battle zone, both teams.
    fn spawn_battle_zone_minions(&mut self) {
        let spawns: Vec<(Hex, Team, MinionKind)> = self
            .board
            .spaces_in_zone(self.battle_zone)
            .filter_map(|(h, s)| match s.spawn {
                Some(SpawnPoint::Minion { team, kind }) => Some((h, team, kind)),
                _ => None,
            })
            .collect();
        for (hex, team, kind) in spawns {
            self.add_unit(team, UnitKind::Minion { kind }, hex);
        }
    }

    pub(crate) fn add_unit(&mut self, team: Team, kind: UnitKind, hex: Hex) -> UnitId {
        let id = UnitId(self.next_unit);
        self.next_unit += 1;
        self.units.insert(
            id,
            Unit {
                id,
                team,
                kind,
                hex: Some(hex),
            },
        );
        id
    }

    pub(crate) fn log(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
    }

    pub const fn team_index(team: Team) -> usize {
        match team {
            Team::Red => 0,
            Team::Blue => 1,
        }
    }

    pub fn player(&self, id: PlayerId) -> &Player {
        &self.players[id.0 as usize]
    }

    pub(crate) fn player_mut(&mut self, id: PlayerId) -> &mut Player {
        &mut self.players[id.0 as usize]
    }

    pub fn player_of_unit(&self, unit: UnitId) -> Option<&Player> {
        self.players.iter().find(|p| p.hero == unit)
    }

    pub fn hero_hex(&self, id: PlayerId) -> Option<Hex> {
        self.units[&self.player(id).hero].hex
    }

    pub fn unit_at(&self, hex: Hex) -> Option<&Unit> {
        self.units.values().find(|u| u.hex == Some(hex))
    }

    /// Terrain, units and tokens all block (rulebook, "Obstacle").
    pub fn is_obstacle(&self, hex: Hex) -> bool {
        match self.board.space(hex) {
            None => true,
            Some(s) => s.terrain || self.unit_at(hex).is_some(),
        }
    }

    pub fn minions_in_battle_zone(&self, team: Team) -> Vec<UnitId> {
        self.units
            .values()
            .filter(|u| {
                !u.is_hero()
                    && u.team == team
                    && u.hex
                        .and_then(|h| self.board.space(h))
                        .is_some_and(|s| s.zone == self.battle_zone)
            })
            .map(|u| u.id)
            .collect()
    }

    /// A heavy minion is immune while a friendly non-heavy minion stands in
    /// the battle zone.
    pub fn is_immune(&self, unit: UnitId) -> bool {
        let u = &self.units[&unit];
        u.minion_kind() == Some(MinionKind::Heavy)
            && self
                .minions_in_battle_zone(u.team)
                .iter()
                .any(|m| self.units[m].minion_kind() != Some(MinionKind::Heavy))
    }

    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::GameOver { .. })
    }

    pub fn snapshot(&self) -> Snapshot {
        let spaces = self
            .board
            .spaces
            .iter()
            .map(|(hex, s)| SpaceView {
                hex: *hex,
                zone: s.zone,
                terrain: s.terrain,
                spawn: s.spawn,
            })
            .collect();
        let zone_adjacency = self
            .board
            .zone_adjacency()
            .into_iter()
            .map(|(a, b)| [a, b])
            .collect();
        let players = self.players.iter().map(|p| self.player_view(p)).collect();
        Snapshot {
            zones: self.board.zones.clone(),
            spaces,
            zone_adjacency,
            units: self.units.values().cloned().collect(),
            battle_zone: self.battle_zone,
            round: self.round,
            turn: self.turn,
            waves: self.waves,
            lives: LivesView {
                red: self.lives[0],
                blue: self.lives[1],
            },
            tie_breaker: self.tie_breaker,
            phase: self.phase.clone(),
            players,
            actions: self.legal_actions(),
            log: self.log.clone(),
        }
    }

    fn player_view(&self, p: &Player) -> PlayerView {
        let cards = |ids: &[CardId]| {
            ids.iter()
                .map(|c| p.card(&self.catalog, *c).clone())
                .collect()
        };
        let items = [
            ItemIcon::Attack,
            ItemIcon::Defense,
            ItemIcon::Initiative,
            ItemIcon::Movement,
            ItemIcon::Range,
            ItemIcon::Area,
        ]
        .into_iter()
        .filter_map(|icon| {
            let n = p.item_bonus(&self.catalog, icon);
            (n > 0).then_some((icon, n))
        })
        .collect();
        PlayerView {
            id: p.id,
            team: p.team,
            hero_key: p.hero_key.clone(),
            hero_name: p.hero_name.clone(),
            hero: p.hero,
            level: p.level,
            coins: p.coins,
            lowest_tier: p.lowest_tier(&self.catalog),
            hand: cards(&p.hand),
            played: p.played.map(|c| p.card(&self.catalog, c).clone()),
            resolved: cards(&p.resolved),
            discard: cards(&p.discard),
            deck: cards(&p.deck),
            items,
            ultimate: p.ultimate.map(|c| p.card(&self.catalog, c).clone()),
        }
    }
}

/// The lane zone halfway between the thrones: the starting battle zone.
fn middle_lane_zone(board: &Board) -> Result<ZoneId, SetupError> {
    let mut lane: Vec<&Zone> = board
        .zones
        .iter()
        .filter(|z| z.lane_index.is_some())
        .collect();
    lane.sort_by_key(|z| z.lane_index);
    if lane.len().is_multiple_of(2) {
        return Err(SetupError::NoMiddleZone);
    }
    let middle = lane[lane.len() / 2];
    debug_assert_eq!(middle.kind, ZoneKind::Lane);
    Ok(middle.id)
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub zones: Vec<Zone>,
    pub spaces: Vec<SpaceView>,
    pub zone_adjacency: Vec<[ZoneId; 2]>,
    pub units: Vec<Unit>,
    pub battle_zone: ZoneId,
    pub round: u16,
    pub turn: u8,
    pub waves: u8,
    pub lives: LivesView,
    pub tie_breaker: Team,
    pub phase: Phase,
    pub players: Vec<PlayerView>,
    /// Everything that can legally happen next.
    pub actions: Vec<Action>,
    pub log: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LivesView {
    pub red: u8,
    pub blue: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpaceView {
    pub hex: Hex,
    pub zone: ZoneId,
    pub terrain: bool,
    pub spawn: Option<SpawnPoint>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlayerView {
    pub id: PlayerId,
    pub team: Team,
    pub hero_key: String,
    pub hero_name: String,
    pub hero: UnitId,
    pub level: u8,
    pub coins: u8,
    pub lowest_tier: u8,
    pub hand: Vec<CardDef>,
    pub played: Option<CardDef>,
    pub resolved: Vec<CardDef>,
    pub discard: Vec<CardDef>,
    pub deck: Vec<CardDef>,
    pub items: Vec<(ItemIcon, u8)>,
    pub ultimate: Option<CardDef>,
}

#[allow(dead_code)]
fn color_of(card: &CardDef) -> Color {
    card.color
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teaching_game_seats_four_heroes_on_their_thrones() {
        let game = Game::teaching_game();
        assert_eq!(game.players.len(), 4);
        for p in &game.players {
            let hero = &game.units[&p.hero];
            let hex = hero.hex.expect("on the board at start");
            let space = game.board.space(hex).unwrap();
            assert_eq!(space.zone, game.board.throne(p.team).id);
            assert_eq!(space.spawn, Some(SpawnPoint::Hero { team: p.team }));
            assert_eq!(p.hand.len(), 5, "{} starts with 5 cards", p.hero_name);
            assert_eq!(p.deck.len(), 13, "{} keeps 13 in the deck", p.hero_name);
            assert_eq!(p.lowest_tier(&game.catalog), 1);
        }
        assert_eq!(game.players[0].hero_name, "Wasp the Warmaiden");
    }

    #[test]
    fn heroes_in_the_same_seat_start_on_twin_spaces() {
        let mut setup = Setup::teaching_game();
        setup.red = vec!["wasp".into(), "xargatha".into(), "arien".into()];
        setup.blue = setup.red.clone();
        let game = Game::new(&setup).unwrap();
        let start = |team| -> Vec<Hex> {
            game.players
                .iter()
                .filter(|p| p.team == team)
                .map(|p| game.units[&p.hero].hex.unwrap())
                .collect()
        };
        let (red, blue) = (start(Team::Red), start(Team::Blue));
        // Twins add up to the same point: twice the board's centre.
        let double_centre = red[0] + blue[0];
        for (r, b) in red.iter().zip(&blue) {
            assert_eq!(*r + *b, double_centre, "{r:?} and {b:?} are not twins");
        }
    }

    #[test]
    fn every_minion_spawn_in_the_battle_zone_is_filled() {
        let game = Game::teaching_game();
        for (hex, space) in game.board.spaces_in_zone(game.battle_zone) {
            let Some(SpawnPoint::Minion { team, kind }) = space.spawn else {
                continue;
            };
            let unit = game.unit_at(hex).expect("minion on its spawn point");
            assert_eq!(unit.team, team);
            assert_eq!(unit.minion_kind(), Some(kind));
        }
        assert_eq!(game.minions_in_battle_zone(Team::Red).len(), 6);
        assert_eq!(game.minions_in_battle_zone(Team::Blue).len(), 6);
    }

    #[test]
    fn no_two_units_share_a_hex() {
        let game = Game::teaching_game();
        let mut seen = std::collections::BTreeSet::new();
        for u in game.units.values() {
            assert!(seen.insert(u.hex.unwrap()), "{:?} doubled up", u.hex);
        }
    }

    #[test]
    fn heavy_minion_is_immune_while_friends_stand() {
        let mut game = Game::teaching_game();
        let red = game.minions_in_battle_zone(Team::Red);
        let heavy = *red
            .iter()
            .find(|m| game.units[m].minion_kind() == Some(MinionKind::Heavy))
            .unwrap();
        assert!(game.is_immune(heavy));
        for m in red.iter().filter(|m| **m != heavy) {
            game.units.remove(m);
        }
        assert!(!game.is_immune(heavy));
    }

    #[test]
    fn units_and_terrain_are_obstacles() {
        let game = Game::teaching_game();
        assert!(game.is_obstacle(game.hero_hex(PlayerId(0)).unwrap()));
        let (terrain, _) = game.board.spaces.iter().find(|(_, s)| s.terrain).unwrap();
        assert!(game.is_obstacle(*terrain));
        assert!(game.is_obstacle(Hex::new(999, 999)), "off-board blocks");
    }

    #[test]
    fn too_many_heroes_for_a_throne_is_an_error() {
        let mut setup = Setup::teaching_game();
        setup.red.extend(["tigerclaw", "sabina"].map(String::from));
        let err = Game::new(&setup).unwrap_err();
        assert!(matches!(
            err,
            SetupError::TooManyHeroes {
                team: Team::Red,
                needed: 4,
                ..
            }
        ));
    }

    #[test]
    fn snapshot_serialises_to_json() {
        let json = serde_json::to_value(Game::teaching_game().snapshot()).unwrap();
        assert_eq!(json["units"].as_array().unwrap().len(), 16);
        assert_eq!(json["players"].as_array().unwrap().len(), 4);
        assert_eq!(json["phase"]["type"], "Play");
        assert_eq!(
            json["actions"].as_array().unwrap().len(),
            20,
            "5 cards x 4 players"
        );
    }
}

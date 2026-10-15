//! Where a hero can go: by stepping, or by fast travel between zones.

use crate::board::{Team, ZoneId};
use crate::game::Game;
use crate::hex::Hex;
use std::collections::{BTreeSet, VecDeque};

impl Game {
    /// Every space reachable in at most `steps` moves through empty spaces.
    /// The start itself is not included.
    pub fn reachable(&self, from: Hex, steps: u8) -> BTreeSet<Hex> {
        let mut seen = BTreeSet::from([from]);
        let mut out = BTreeSet::new();
        let mut queue = VecDeque::from([(from, 0u8)]);
        while let Some((hex, d)) = queue.pop_front() {
            if d == steps {
                continue;
            }
            for n in hex.neighbors() {
                if seen.contains(&n) || self.is_obstacle(n) {
                    continue;
                }
                seen.insert(n);
                out.insert(n);
                queue.push_back((n, d + 1));
            }
        }
        out
    }

    /// Fast travel: any empty space in the hero's zone or an adjacent one,
    /// as long as neither zone holds an enemy unit.
    pub fn fast_travel_targets(&self, from: Hex, team: Team) -> Vec<Hex> {
        let Some(here) = self.board.space(from).map(|s| s.zone) else {
            return Vec::new();
        };
        if self.zone_has_enemies(here, team) {
            return Vec::new();
        }
        let mut zones = vec![here];
        zones.extend(
            self.board
                .zones
                .iter()
                .map(|z| z.id)
                .filter(|z| self.board.zones_adjacent(here, *z)),
        );
        zones
            .into_iter()
            .filter(|z| !self.zone_has_enemies(*z, team))
            .flat_map(|z| self.empty_spaces_in_zone(z))
            .filter(|h| *h != from)
            .collect()
    }

    pub fn zone_has_enemies(&self, zone: ZoneId, team: Team) -> bool {
        self.units.values().any(|u| {
            u.team != team
                && u.hex
                    .and_then(|h| self.board.space(h))
                    .is_some_and(|s| s.zone == zone)
        })
    }

    pub fn empty_spaces_in_zone(&self, zone: ZoneId) -> Vec<Hex> {
        self.board
            .spaces_in_zone(zone)
            .map(|(h, _)| h)
            .filter(|h| !self.is_obstacle(*h))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn reachable_stops_at_obstacles_and_range() {
        let game = Game::teaching_game();
        let hero = game.players[0].hero;
        let from = game.units[&hero].hex.unwrap();
        let one = game.reachable(from, 1);
        let two = game.reachable(from, 2);
        assert!(!one.is_empty());
        assert!(one.is_subset(&two));
        assert!(two.len() > one.len());
        for h in &two {
            assert!(!game.is_obstacle(*h));
            assert!(from.distance(*h) <= 2);
        }
    }

    #[test]
    fn fast_travel_from_a_throne_reaches_the_outpost() {
        let game = Game::teaching_game();
        let hero = game.players[0].hero;
        let from = game.units[&hero].hex.unwrap();
        let team = game.units[&hero].team;
        let targets = game.fast_travel_targets(from, team);
        let zones: BTreeSet<ZoneId> = targets
            .iter()
            .map(|h| game.board.space(*h).unwrap().zone)
            .collect();
        assert!(zones.contains(&game.board.throne(team).id));
        assert!(zones.len() >= 2, "throne plus the outpost");
        assert!(!zones.contains(&game.battle_zone), "enemy minions block it");
    }
}

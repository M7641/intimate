//! End of round: minion battle, lane push, retrieving cards, leveling up.

use crate::board::{MinionKind, SpawnPoint, Team, ZoneKind};
use crate::card::{CardId, Color};
use crate::game::Game;
use crate::hex::Hex;
use crate::player::PlayerId;
use crate::rules::{MAX_LEVEL, level_up_cost};
use crate::turn::{Action, Phase};
use crate::unit::UnitKind;

impl Game {
    pub(crate) fn end_round(&mut self) {
        self.minion_battle();
        if self.is_over() {
            return;
        }
        for p in &mut self.players {
            p.hand.append(&mut p.resolved);
            p.hand.append(&mut p.discard);
            p.hand.sort();
        }
        self.log("Cards retrieved.".to_string());
        for i in 0..self.players.len() as u8 {
            self.buy_level_ups(PlayerId(i));
        }
        self.continue_level_ups();
    }

    /// The side with fewer minions in the battle zone loses the difference.
    /// Simplification: the engine picks which, melee first, heavy last.
    fn minion_battle(&mut self) {
        let red = self.minions_in_battle_zone(Team::Red);
        let blue = self.minions_in_battle_zone(Team::Blue);
        if red.len() == blue.len() {
            self.log(format!(
                "Minion battle: {} against {}, no effect.",
                red.len(),
                blue.len()
            ));
            return;
        }
        let diff = red.len().abs_diff(blue.len());
        let (loser, mut losers) = if red.len() < blue.len() {
            (Team::Red, red)
        } else {
            (Team::Blue, blue)
        };
        let rank = |k: Option<MinionKind>| match k {
            Some(MinionKind::Melee) => 0,
            Some(MinionKind::Ranged) => 1,
            _ => 2,
        };
        losers.sort_by_key(|m| rank(self.units[m].minion_kind()));
        self.log(format!(
            "Minion battle: {loser:?} is outnumbered and removes {diff} minion(s)."
        ));
        for m in losers.into_iter().take(diff) {
            if self.is_over() {
                return;
            }
            self.defeat_minion(m, None);
        }
    }

    /// One side has no minions left in the battle zone: the other pushes.
    pub(crate) fn push_lane(&mut self, winner: Team) {
        self.waves -= 1;
        self.log(format!(
            "{winner:?} pushes the lane. {} wave counter(s) left.",
            self.waves
        ));
        if self.waves == 0 {
            self.phase = Phase::GameOver {
                winner,
                reason: "won the last push".into(),
            };
            self.log(format!("{winner:?} wins the last push."));
            return;
        }
        let loser = winner.opponent();
        let old = self.battle_zone;
        let index = self
            .board
            .zone(old)
            .lane_index
            .expect("battle zone is on the lane");
        let next_index = if loser == Team::Blue {
            index + 1
        } else {
            index - 1
        };
        let next = self
            .board
            .zones
            .iter()
            .find(|z| z.lane_index == Some(next_index))
            .expect("lane is contiguous")
            .id;
        if self.board.zone(next).kind == (ZoneKind::Throne { team: loser }) {
            self.phase = Phase::GameOver {
                winner,
                reason: format!("captured the {loser:?} throne"),
            };
            self.log(format!("{winner:?} captures the {loser:?} throne."));
            return;
        }
        let leftovers: Vec<_> = self
            .units
            .values()
            .filter(|u| {
                !u.is_hero()
                    && u.hex
                        .and_then(|h| self.board.space(h))
                        .is_some_and(|s| s.zone == old)
            })
            .map(|u| u.id)
            .collect();
        for m in leftovers {
            self.units.remove(&m);
        }
        self.battle_zone = next;
        self.respawn_minions();
        self.log(format!(
            "The battle zone is now {}.",
            self.board.zone(next).name
        ));
    }

    /// One minion per spawn point in the battle zone. A spawn point under a
    /// hero sends its minion to the nearest empty space in the zone.
    fn respawn_minions(&mut self) {
        let zone = self.battle_zone;
        let spawns: Vec<(Hex, Team, MinionKind)> = self
            .board
            .spaces_in_zone(zone)
            .filter_map(|(h, s)| match s.spawn {
                Some(SpawnPoint::Minion { team, kind }) => Some((h, team, kind)),
                _ => None,
            })
            .collect();
        for (hex, team, kind) in spawns {
            let target = if self.is_obstacle(hex) {
                self.empty_spaces_in_zone(zone)
                    .into_iter()
                    .min_by_key(|h| (hex.distance(*h), *h))
            } else {
                Some(hex)
            };
            if let Some(at) = target {
                self.add_unit(team, UnitKind::Minion { kind }, at);
            }
        }
    }

    // ── Leveling up ──────────────────────────────────────────────────

    /// Buy every level the coins allow. Each level below 8 queues one
    /// upgrade choice; level 8 unlocks the ultimate.
    fn buy_level_ups(&mut self, player: PlayerId) {
        loop {
            let p = self.player(player);
            let next = p.level + 1;
            if next > MAX_LEVEL || p.coins < level_up_cost(next) {
                break;
            }
            let catalog = self.catalog.clone();
            let p = self.player_mut(player);
            p.coins -= level_up_cost(next);
            p.level = next;
            p.leveled_this_round = true;
            if next == MAX_LEVEL {
                let ult = p
                    .deck
                    .iter()
                    .copied()
                    .find(|c| p.card(&catalog, *c).color == Color::Purple);
                p.deck.retain(|c| Some(*c) != ult);
                p.ultimate = ult;
                self.log(format!(
                    "{} reaches level 8 and unlocks the ultimate.",
                    self.name(player)
                ));
            } else {
                self.pending_level_ups.push_back(player);
                self.log(format!("{} levels up to {next}.", self.name(player)));
            }
        }
    }

    pub(crate) fn continue_level_ups(&mut self) {
        match self.pending_level_ups.front() {
            Some(p) => self.phase = Phase::LevelUp { player: *p },
            None => self.finish_round(),
        }
    }

    /// Lowest-tier non-basic cards in hand, each with its two next-tier
    /// siblings in the deck.
    pub(crate) fn upgrade_actions(&self, player: PlayerId) -> Vec<Action> {
        let p = self.player(player);
        let tier_of = |c: &CardId| p.card(&self.catalog, *c).tier;
        let Some(lowest) = p.hand.iter().filter_map(tier_of).min() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for card_out in p.hand.iter().filter(|c| tier_of(c) == Some(lowest)) {
            let color = p.card(&self.catalog, *card_out).color;
            for take in p.deck.iter() {
                let def = p.card(&self.catalog, *take);
                if def.color == color && def.tier == Some(lowest + 1) {
                    out.push(Action::Upgrade {
                        player,
                        card_out: *card_out,
                        take: *take,
                    });
                }
            }
        }
        out
    }

    pub(crate) fn upgrade(&mut self, player: PlayerId, card_out: CardId, take: CardId) {
        let catalog = self.catalog.clone();
        let p = self.player_mut(player);
        let taken = p.card(&catalog, take).clone();
        let sibling = p
            .deck
            .iter()
            .copied()
            .find(|c| {
                let d = p.card(&catalog, *c);
                *c != take && d.color == taken.color && d.tier == taken.tier
            })
            .expect("every tier has two cards");
        p.hand.retain(|c| *c != card_out);
        p.deck.push(card_out);
        p.deck.retain(|c| *c != take && *c != sibling);
        p.hand.push(take);
        p.hand.sort();
        p.item_cards.push(sibling);
        let item = p.card(&catalog, sibling).item;
        self.pending_level_ups.pop_front();
        self.log(format!(
            "{} upgrades {} to {} and gains a {:?} item.",
            self.name(player),
            catalog.card(&self.player(player).hero_key, card_out).name,
            taken.name,
            item.expect("tier II+ cards carry an item icon")
        ));
    }

    /// Pity coins, then a new round.
    fn finish_round(&mut self) {
        for p in &mut self.players {
            if !p.leveled_this_round {
                p.coins += 1;
            }
            p.leveled_this_round = false;
        }
        self.round += 1;
        self.turn = 1;
        self.phase = Phase::Play;
        self.log(format!("Round {} begins.", self.round));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outnumbered_side_loses_minions_heavy_last() {
        let mut game = Game::teaching_game();
        // Remove two Blue melee minions; the battle then removes two Red.
        let blue = game.minions_in_battle_zone(Team::Blue);
        let melee: Vec<_> = blue
            .iter()
            .filter(|m| game.units[m].minion_kind() == Some(MinionKind::Melee))
            .take(2)
            .copied()
            .collect();
        for m in melee {
            game.units.remove(&m);
        }
        game.minion_battle();
        assert_eq!(game.minions_in_battle_zone(Team::Red).len(), 6);
        let blue = game.minions_in_battle_zone(Team::Blue);
        assert_eq!(blue.len(), 2, "4 minus the difference of 2");
        assert!(
            blue.iter()
                .any(|m| game.units[m].minion_kind() == Some(MinionKind::Heavy))
        );
    }

    #[test]
    fn push_moves_the_battle_zone_and_respawns_minions() {
        let mut game = Game::teaching_game();
        let old = game.battle_zone;
        for m in game.minions_in_battle_zone(Team::Blue) {
            game.units.remove(&m);
        }
        game.push_lane(Team::Red);
        assert_eq!(game.waves, 2);
        let new = game.battle_zone;
        assert_ne!(new, old);
        assert_eq!(game.board.zone(new).name, "Blue Outpost");
        assert_eq!(
            game.minions_in_battle_zone(Team::Blue).len(),
            6,
            "defender's outpost"
        );
        assert_eq!(
            game.minions_in_battle_zone(Team::Red).len(),
            5,
            "attacker gets fewer"
        );
        assert!(!game.is_over());
    }

    #[test]
    fn pushing_into_a_throne_wins() {
        let mut game = Game::teaching_game();
        game.push_lane(Team::Red);
        game.push_lane(Team::Red);
        assert!(matches!(
            game.phase,
            Phase::GameOver {
                winner: Team::Red,
                ..
            }
        ));
    }

    #[test]
    fn last_wave_counter_ends_the_game() {
        let mut game = Game::teaching_game();
        game.waves = 1;
        game.push_lane(Team::Blue);
        assert!(matches!(
            game.phase,
            Phase::GameOver {
                winner: Team::Blue,
                ..
            }
        ));
    }

    #[test]
    fn leveling_up_upgrades_a_card_and_grants_an_item() {
        let mut game = Game::teaching_game();
        game.players[0].coins = 3; // level 2 costs 1, level 3 costs 2
        game.buy_level_ups(PlayerId(0));
        assert_eq!(game.player(PlayerId(0)).level, 3);
        assert_eq!(game.player(PlayerId(0)).coins, 0);
        assert_eq!(game.pending_level_ups.len(), 2);
        game.continue_level_ups();
        assert_eq!(
            game.phase,
            Phase::LevelUp {
                player: PlayerId(0)
            }
        );
        let actions = game.legal_actions();
        assert_eq!(actions.len(), 6, "3 tier I cards x 2 tier II choices");
        let first = actions[0].clone();
        game.apply(first).unwrap();
        let p = game.player(PlayerId(0));
        assert_eq!(p.hand.len(), 5);
        assert_eq!(p.item_cards.len(), 1);
        assert_eq!(p.deck.len(), 12, "one card back, two out");
        assert_eq!(
            game.phase,
            Phase::LevelUp {
                player: PlayerId(0)
            }
        );
        // Second upgrade must pick another tier I card, not the fresh tier II.
        for a in game.legal_actions() {
            let Action::Upgrade { card_out, .. } = a else {
                panic!()
            };
            assert_eq!(p.card(&game.catalog, card_out).tier, Some(1));
        }
    }

    #[test]
    fn level_eight_unlocks_the_ultimate() {
        let mut game = Game::teaching_game();
        game.players[0].coins = 28; // 1+2+...+7
        game.buy_level_ups(PlayerId(0));
        assert_eq!(game.player(PlayerId(0)).level, 8);
        assert!(game.player(PlayerId(0)).ultimate.is_some());
        assert_eq!(game.pending_level_ups.len(), 6);
    }
}

//! Attack and defense resolution, and what a defeat costs.

use crate::board::{MinionKind, Team};
use crate::card::{CardId, ItemIcon};
use crate::game::Game;
use crate::hex::Hex;
use crate::player::PlayerId;
use crate::rules::{assist_coins, defeat_coins, life_cost, minion_bounty};
use crate::turn::Phase;
use crate::unit::UnitId;

impl Game {
    /// Perform the attack. Returns `true` when it is fully resolved, `false`
    /// when a hero still has to decide whether to defend.
    pub(crate) fn attack(&mut self, attacker: PlayerId, target: UnitId) -> bool {
        let card = self.played_card(attacker);
        let attack_total = card.attack_value().unwrap_or(0)
            + self
                .player(attacker)
                .item_bonus(&self.catalog, ItemIcon::Attack);
        let card_name = card.name.clone();
        let target_unit = self.units[&target].clone();
        match self.player_of_unit(target).map(|p| p.id) {
            None => {
                let kind = target_unit
                    .minion_kind()
                    .expect("non-hero units are minions");
                self.log(format!(
                    "{} attacks a {:?} {:?} minion with {card_name} and defeats it.",
                    self.name(attacker),
                    target_unit.team,
                    kind
                ));
                self.defeat_minion(target, Some(attacker));
                true
            }
            Some(defender) => {
                self.log(format!(
                    "{} attacks {} with {card_name}: attack total {attack_total}.",
                    self.name(attacker),
                    self.name(defender)
                ));
                self.phase = Phase::Defend {
                    attacker,
                    defender,
                    attack_total,
                };
                false
            }
        }
    }

    /// Cards in hand with a defense value, plus taking the hit.
    pub(crate) fn defend_actions(&self, defender: PlayerId) -> Vec<crate::turn::Action> {
        use crate::turn::Action;
        let p = self.player(defender);
        let mut out: Vec<Action> = p
            .hand
            .iter()
            .filter(|c| p.card(&self.catalog, **c).defense_value().is_some())
            .map(|c| Action::Defend {
                player: defender,
                card: Some(*c),
            })
            .collect();
        out.push(Action::Defend {
            player: defender,
            card: None,
        });
        out
    }

    pub(crate) fn resolve_defense(&mut self, defender: PlayerId, card: Option<CardId>) {
        let Phase::Defend {
            attacker,
            attack_total,
            ..
        } = self.phase.clone()
        else {
            unreachable!("resolve_defense outside Defend");
        };
        let survived = match card {
            None => {
                self.log(format!("{} does not defend.", self.name(defender)));
                false
            }
            Some(card) => {
                let p = self.player_mut(defender);
                p.hand.retain(|c| *c != card);
                p.discard.push(card);
                let def = self.player(defender).card(&self.catalog, card);
                let name = def.name.clone();
                let base = def.defense_value().unwrap_or(0);
                let items = self
                    .player(defender)
                    .item_bonus(&self.catalog, ItemIcon::Defense);
                let hex = self
                    .hero_hex(defender)
                    .expect("defender stands on the board");
                let modifiers = self.minion_defense_modifiers(hex, self.player(defender).team);
                let total = (i32::from(base) + i32::from(items) + modifiers).max(0) as u8;
                let ok = total >= attack_total;
                self.log(format!(
                    "{} defends with {name}: {base} + {items} items {modifiers:+} minions = {total} vs {attack_total}: {}.",
                    self.name(defender),
                    if ok { "holds" } else { "falls" }
                ));
                ok
            }
        };
        if !survived {
            self.defeat_hero(defender, attacker);
        }
        if !self.is_over() {
            self.phase = Phase::Act { player: attacker };
            self.finish_action(attacker);
        }
    }

    /// +1 per adjacent friendly melee/heavy, -1 per adjacent enemy
    /// melee/heavy, -1 per enemy ranged minion within 2 spaces.
    pub fn minion_defense_modifiers(&self, hex: Hex, team: Team) -> i32 {
        let mut total = 0;
        for u in self.units.values() {
            let (Some(kind), Some(at)) = (u.minion_kind(), u.hex) else {
                continue;
            };
            let d = hex.distance(at);
            match (kind, u.team == team) {
                (MinionKind::Melee | MinionKind::Heavy, true) if d == 1 => total += 1,
                (MinionKind::Melee | MinionKind::Heavy, false) if d == 1 => total -= 1,
                (MinionKind::Ranged, false) if d <= 2 => total -= 1,
                _ => {}
            }
        }
        total
    }

    /// Remove a minion. With an attacker, pay the bounty. Either way, an
    /// emptied side pushes the lane.
    pub(crate) fn defeat_minion(&mut self, minion: UnitId, attacker: Option<PlayerId>) {
        let unit = self.units.remove(&minion).expect("minion exists");
        if let (Some(attacker), Some(kind)) = (attacker, unit.minion_kind()) {
            self.player_mut(attacker).coins += minion_bounty(kind);
        }
        if self.minions_in_battle_zone(unit.team).is_empty() {
            self.push_lane(unit.team.opponent());
        }
    }

    pub(crate) fn defeat_hero(&mut self, defender: PlayerId, attacker: PlayerId) {
        let hero = self.player(defender).hero;
        self.units.get_mut(&hero).unwrap().hex = None;
        let p = self.player_mut(defender);
        if let Some(card) = p.played.take() {
            p.resolved.push(card);
        }
        let level = p.level;
        let tier = self.player(defender).lowest_tier(&self.catalog);
        let team = self.player(defender).team;
        let attacking_team = self.player(attacker).team;

        self.player_mut(attacker).coins += defeat_coins(level);
        let assists: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|p| p.team == attacking_team && p.id != attacker)
            .map(|p| p.id)
            .collect();
        for a in assists {
            self.player_mut(a).coins += assist_coins(level);
        }
        let cost = life_cost(tier);
        let lives = &mut self.lives[Game::team_index(team)];
        *lives = lives.saturating_sub(cost);
        let left = *lives;
        self.log(format!(
            "{} is defeated. {:?} spends {cost} life counter(s), {left} left.",
            self.name(defender),
            team
        ));
        if left == 0 {
            self.phase = Phase::GameOver {
                winner: attacking_team,
                reason: format!("{team:?} has no life counters left"),
            };
            self.log(format!(
                "{attacking_team:?} wins: {team:?} is out of lives."
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Color;
    use crate::turn::Action;

    /// Put every hero next to the battle zone and give the actor a Red card.
    fn arena() -> (Game, PlayerId) {
        let mut game = Game::teaching_game();
        // Wasp (player 0, Red) stands next to a Blue melee minion.
        let blue_melee = *game
            .minions_in_battle_zone(Team::Blue)
            .iter()
            .find(|m| game.units[m].minion_kind() == Some(MinionKind::Melee))
            .unwrap();
        let next_to = game.units[&blue_melee]
            .hex
            .unwrap()
            .neighbors()
            .into_iter()
            .find(|h| !game.is_obstacle(*h))
            .unwrap();
        let wasp = game.players[0].hero;
        game.units.get_mut(&wasp).unwrap().hex = Some(next_to);
        for i in 0..4u8 {
            let player = PlayerId(i);
            let color = if i == 0 { Color::Gold } else { Color::Green };
            let p = game.player(player);
            let card = *p
                .hand
                .iter()
                .find(|c| p.card(&game.catalog, **c).color == color)
                .unwrap();
            game.apply(Action::PlayCard { player, card }).unwrap();
        }
        assert_eq!(
            game.phase,
            Phase::Act {
                player: PlayerId(0)
            },
            "Gold acts first"
        );
        (game, PlayerId(0))
    }

    #[test]
    fn defeating_a_minion_pays_two_coins() {
        let (mut game, wasp) = arena();
        let attack = game
            .legal_actions()
            .into_iter()
            .find(|a| matches!(a, Action::Attack { .. }))
            .expect("a minion is adjacent");
        let before = game.units.len();
        game.apply(attack).unwrap();
        assert_eq!(game.units.len(), before - 1);
        assert_eq!(game.player(wasp).coins, 2);
    }

    #[test]
    fn heavy_minion_cannot_be_targeted_while_shielded() {
        let (mut game, _) = arena();
        let heavy = *game
            .minions_in_battle_zone(Team::Blue)
            .iter()
            .find(|m| game.units[m].minion_kind() == Some(MinionKind::Heavy))
            .unwrap();
        let wasp = game.players[0].hero;
        let near_heavy = game.units[&heavy]
            .hex
            .unwrap()
            .neighbors()
            .into_iter()
            .find(|h| !game.is_obstacle(*h))
            .unwrap();
        game.units.get_mut(&wasp).unwrap().hex = Some(near_heavy);
        let targets: Vec<UnitId> = game
            .legal_actions()
            .into_iter()
            .filter_map(|a| match a {
                Action::Attack { target, .. } => Some(target),
                _ => None,
            })
            .collect();
        assert!(!targets.contains(&heavy));
    }

    #[test]
    fn a_hero_defends_with_a_card_or_falls() {
        let (mut game, wasp) = arena();
        // Bring Arien (player 2, Blue) next to Wasp, away from minions.
        let arien = game.players[2].hero;
        let wasp_hex = game.hero_hex(wasp).unwrap();
        let spot = wasp_hex
            .neighbors()
            .into_iter()
            .find(|h| !game.is_obstacle(*h))
            .unwrap();
        game.units.get_mut(&arien).unwrap().hex = Some(spot);
        game.apply(Action::Attack {
            player: wasp,
            target: arien,
        })
        .unwrap();
        let Phase::Defend {
            attack_total,
            defender,
            ..
        } = game.phase.clone()
        else {
            panic!("hero target opens a defense");
        };
        assert_eq!(defender, PlayerId(2));
        assert!(attack_total > 0);
        // Take the hit.
        let lives_before = game.lives[1];
        game.apply(Action::Defend {
            player: defender,
            card: None,
        })
        .unwrap();
        assert_eq!(game.units[&arien].hex, None, "Arien leaves the board");
        assert_eq!(
            game.lives[1],
            lives_before - 1,
            "tier I hero costs one life"
        );
        assert_eq!(game.player(wasp).coins, 1, "level 1 bounty");
        assert_eq!(game.player(PlayerId(1)).coins, 1, "assist coin");
        assert!(
            game.player(wasp).played.is_none(),
            "attacker's card resolved"
        );
    }

    #[test]
    fn a_strong_enough_defense_holds() {
        let (mut game, wasp) = arena();
        let arien = game.players[2].hero;
        let spot = game
            .hero_hex(wasp)
            .unwrap()
            .neighbors()
            .into_iter()
            .find(|h| !game.is_obstacle(*h))
            .unwrap();
        game.units.get_mut(&arien).unwrap().hex = Some(spot);
        game.apply(Action::Attack {
            player: wasp,
            target: arien,
        })
        .unwrap();
        let Phase::Defend {
            attack_total,
            defender,
            ..
        } = game.phase.clone()
        else {
            panic!()
        };
        // Pick the best defense card in hand.
        let p = game.player(defender);
        let best = *p
            .hand
            .iter()
            .max_by_key(|c| p.card(&game.catalog, **c).defense_value().unwrap_or(0))
            .unwrap();
        let value = p.card(&game.catalog, best).defense_value().unwrap();
        let modifiers = game.minion_defense_modifiers(spot, Team::Blue);
        let expect_hold = i32::from(value) + modifiers >= i32::from(attack_total);
        game.apply(Action::Defend {
            player: defender,
            card: Some(best),
        })
        .unwrap();
        assert_eq!(game.units[&arien].hex.is_some(), expect_hold);
        assert!(
            game.player(defender).discard.contains(&best),
            "defense card is discarded"
        );
    }
}

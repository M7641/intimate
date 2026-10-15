//! The turn machine: phases, actions, what is legal, and what applying an
//! action does. Combat and end-of-round steps live in their own modules.

use crate::board::Team;
use crate::card::{CardId, ItemIcon};
use crate::game::Game;
use crate::hex::Hex;
use crate::player::PlayerId;
use crate::rules::TURNS_PER_ROUND;
use crate::unit::UnitId;
use serde::{Deserialize, Serialize};

/// Whose decision the game waits on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum Phase {
    /// Everyone still holding cards plays one facedown.
    Play,
    /// Same-team initiative tie: the team picks who acts.
    ChooseActor {
        candidates: Vec<PlayerId>,
    },
    /// One player performs one action with their played card.
    Act {
        player: PlayerId,
    },
    /// A hero under attack may discard a card to defend.
    Defend {
        attacker: PlayerId,
        defender: PlayerId,
        attack_total: u8,
    },
    /// A player who bought a level picks the card to upgrade.
    LevelUp {
        player: PlayerId,
    },
    GameOver {
        winner: Team,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    PlayCard {
        player: PlayerId,
        card: CardId,
    },
    /// No cards in hand: announce a pass.
    Pass {
        player: PlayerId,
    },
    ChooseActor {
        player: PlayerId,
    },
    /// A hero off the board returns to a spawn point before acting.
    Respawn {
        player: PlayerId,
        hex: Hex,
    },
    Move {
        player: PlayerId,
        to: Hex,
    },
    FastTravel {
        player: PlayerId,
        to: Hex,
    },
    Attack {
        player: PlayerId,
        target: UnitId,
    },
    Skill {
        player: PlayerId,
    },
    /// Do nothing with the played card.
    Hold {
        player: PlayerId,
    },
    /// `None`: take the hit.
    Defend {
        player: PlayerId,
        card: Option<CardId>,
    },
    /// Return `card_out` to the deck, take `take` (one tier up) into hand;
    /// its sibling becomes an item.
    Upgrade {
        player: PlayerId,
        card_out: CardId,
        take: CardId,
    },
}

impl Action {
    pub fn player(&self) -> PlayerId {
        match self {
            Action::PlayCard { player, .. }
            | Action::Pass { player }
            | Action::ChooseActor { player }
            | Action::Respawn { player, .. }
            | Action::Move { player, .. }
            | Action::FastTravel { player, .. }
            | Action::Attack { player, .. }
            | Action::Skill { player }
            | Action::Hold { player }
            | Action::Defend { player, .. }
            | Action::Upgrade { player, .. } => *player,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ActionError {
    #[error("not legal in the current phase: {0:?}")]
    Illegal(Action),
}

impl Game {
    pub fn legal_actions(&self) -> Vec<Action> {
        match &self.phase {
            Phase::Play => self.play_actions(),
            Phase::ChooseActor { candidates } => candidates
                .iter()
                .map(|p| Action::ChooseActor { player: *p })
                .collect(),
            Phase::Act { player } => self.act_actions(*player),
            Phase::Defend { defender, .. } => self.defend_actions(*defender),
            Phase::LevelUp { player } => self.upgrade_actions(*player),
            Phase::GameOver { .. } => Vec::new(),
        }
    }

    pub fn apply(&mut self, action: Action) -> Result<(), ActionError> {
        if !self.legal_actions().contains(&action) {
            return Err(ActionError::Illegal(action));
        }
        match action {
            Action::PlayCard { player, card } => {
                let p = self.player_mut(player);
                p.hand.retain(|c| *c != card);
                p.played = Some(card);
                self.log(format!("{} plays a card facedown.", self.name(player)));
                self.after_play();
            }
            Action::Pass { player } => {
                self.player_mut(player).passed = true;
                self.log(format!("{} passes (no cards).", self.name(player)));
                self.after_play();
            }
            Action::ChooseActor { player } => {
                self.phase = Phase::Act { player };
            }
            Action::Respawn { player, hex } => {
                let hero = self.player(player).hero;
                self.units.get_mut(&hero).unwrap().hex = Some(hex);
                self.log(format!("{} respawns.", self.name(player)));
            }
            Action::Move { player, to } => {
                let hero = self.player(player).hero;
                self.units.get_mut(&hero).unwrap().hex = Some(to);
                self.log(format!(
                    "{} moves to {}.",
                    self.name(player),
                    self.place(to)
                ));
                self.finish_action(player);
            }
            Action::FastTravel { player, to } => {
                let hero = self.player(player).hero;
                self.units.get_mut(&hero).unwrap().hex = Some(to);
                self.log(format!(
                    "{} fast travels to {}.",
                    self.name(player),
                    self.place(to)
                ));
                self.finish_action(player);
            }
            Action::Attack { player, target } => {
                if self.attack(player, target) {
                    self.finish_action(player);
                }
            }
            Action::Skill { player } => {
                let card = self.played_card(player).name.clone();
                self.log(format!(
                    "{} performs the skill on {card} (card text not applied yet).",
                    self.name(player)
                ));
                self.finish_action(player);
            }
            Action::Hold { player } => {
                self.log(format!("{} holds.", self.name(player)));
                self.finish_action(player);
            }
            Action::Defend { player, card } => {
                self.resolve_defense(player, card);
            }
            Action::Upgrade {
                player,
                card_out,
                take,
            } => {
                self.upgrade(player, card_out, take);
                self.continue_level_ups();
            }
        }
        Ok(())
    }

    // ── Play phase ───────────────────────────────────────────────────

    fn play_actions(&self) -> Vec<Action> {
        let mut out = Vec::new();
        for p in &self.players {
            if p.played.is_some() || p.passed {
                continue;
            }
            if p.hand.is_empty() {
                out.push(Action::Pass { player: p.id });
            } else {
                out.extend(p.hand.iter().map(|c| Action::PlayCard {
                    player: p.id,
                    card: *c,
                }));
            }
        }
        out
    }

    fn after_play(&mut self) {
        let all_in = self.players.iter().all(|p| p.played.is_some() || p.passed);
        if !all_in {
            return;
        }
        let reveals: Vec<String> = self
            .players
            .iter()
            .filter_map(|p| {
                let card = p.card(&self.catalog, p.played?);
                Some(format!(
                    "{}: {} ({})",
                    p.hero_name,
                    card.name,
                    self.initiative_of(p.id)
                ))
            })
            .collect();
        self.log(format!("Cards revealed. {}", reveals.join("; ")));
        self.next_actor();
    }

    /// Card initiative plus initiative items.
    pub fn initiative_of(&self, player: PlayerId) -> u8 {
        let p = self.player(player);
        match p.played {
            Some(c) => {
                p.card(&self.catalog, c).initiative
                    + p.item_bonus(&self.catalog, ItemIcon::Initiative)
            }
            None => 0,
        }
    }

    /// Highest unresolved initiative acts next. Ties across teams go to the
    /// tie-breaker coin's team; ties within a team are that team's choice.
    pub(crate) fn next_actor(&mut self) {
        if self.is_over() {
            return;
        }
        let unresolved: Vec<(PlayerId, u8)> = self
            .players
            .iter()
            .filter(|p| p.played.is_some())
            .map(|p| (p.id, self.initiative_of(p.id)))
            .collect();
        let Some(top) = unresolved.iter().map(|(_, i)| *i).max() else {
            self.end_turn();
            return;
        };
        let tied: Vec<PlayerId> = unresolved
            .iter()
            .filter(|(_, i)| *i == top)
            .map(|(p, _)| *p)
            .collect();
        let teams: Vec<Team> = tied.iter().map(|p| self.player(*p).team).collect();
        let candidates = if teams.iter().all(|t| *t == teams[0]) {
            tied
        } else {
            self.flip_after_actor = true;
            tied.into_iter()
                .filter(|p| self.player(*p).team == self.tie_breaker)
                .collect()
        };
        self.phase = if candidates.len() == 1 {
            Phase::Act {
                player: candidates[0],
            }
        } else {
            Phase::ChooseActor { candidates }
        };
    }

    // ── Act phase ────────────────────────────────────────────────────

    fn act_actions(&self, player: PlayerId) -> Vec<Action> {
        let p = self.player(player);
        let card = self.played_card(player);
        let mut out = vec![Action::Hold { player }];
        let Some(from) = self.hero_hex(player) else {
            out.extend(
                self.respawn_spaces(p.team)
                    .into_iter()
                    .map(|hex| Action::Respawn { player, hex }),
            );
            return out;
        };
        if let Some(mv) = card.movement_value() {
            let steps = mv + p.item_bonus(&self.catalog, ItemIcon::Movement);
            out.extend(
                self.reachable(from, steps)
                    .into_iter()
                    .map(|to| Action::Move { player, to }),
            );
            out.extend(
                self.fast_travel_targets(from, p.team)
                    .into_iter()
                    .map(|to| Action::FastTravel { player, to }),
            );
        }
        if card.attack_value().is_some() {
            let reach = match card.range() {
                Some(r) => r + p.item_bonus(&self.catalog, ItemIcon::Range),
                None => 1,
            };
            out.extend(
                self.attack_targets(p.team, from, reach)
                    .into_iter()
                    .map(|target| Action::Attack { player, target }),
            );
        }
        if card.has_skill() {
            out.push(Action::Skill { player });
        }
        out
    }

    /// Enemy units within `reach` that are not immune.
    fn attack_targets(&self, team: Team, from: Hex, reach: u8) -> Vec<UnitId> {
        self.units
            .values()
            .filter(|u| u.team != team)
            .filter(|u| u.hex.is_some_and(|h| from.distance(h) <= i32::from(reach)))
            .filter(|u| !self.is_immune(u.id))
            .map(|u| u.id)
            .collect()
    }

    /// Empty hero spawn points in the team's throne; failing that, empty
    /// spaces next to one.
    fn respawn_spaces(&self, team: Team) -> Vec<Hex> {
        let throne = self.board.throne(team).id;
        let spawns: Vec<Hex> = self
            .board
            .spaces_in_zone(throne)
            .filter(|(_, s)| s.spawn == Some(crate::board::SpawnPoint::Hero { team }))
            .map(|(h, _)| h)
            .collect();
        let empty: Vec<Hex> = spawns
            .iter()
            .copied()
            .filter(|h| !self.is_obstacle(*h))
            .collect();
        if !empty.is_empty() {
            return empty;
        }
        let mut near: Vec<Hex> = spawns
            .iter()
            .flat_map(|h| h.neighbors())
            .filter(|n| {
                !self.is_obstacle(*n) && self.board.space(*n).is_some_and(|s| s.zone == throne)
            })
            .collect();
        near.sort();
        near.dedup();
        near
    }

    /// The played card becomes resolved; the coin flips if it must; the next
    /// actor is found.
    pub(crate) fn finish_action(&mut self, player: PlayerId) {
        let p = self.player_mut(player);
        if let Some(card) = p.played.take() {
            p.resolved.push(card);
        }
        if self.flip_after_actor {
            self.flip_after_actor = false;
            self.tie_breaker = self.tie_breaker.opponent();
            self.log(format!("Tie breaker flips to {:?}.", self.tie_breaker));
        }
        self.next_actor();
    }

    fn end_turn(&mut self) {
        for p in &mut self.players {
            p.passed = false;
        }
        if self.turn == TURNS_PER_ROUND {
            self.log(format!(
                "Turn {} ends. Round {} ends.",
                self.turn, self.round
            ));
            self.end_round();
        } else {
            self.turn += 1;
            self.log(format!("Turn {} begins.", self.turn));
            self.phase = Phase::Play;
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    pub(crate) fn played_card(&self, player: PlayerId) -> &crate::card::CardDef {
        let p = self.player(player);
        p.card(
            &self.catalog,
            p.played.expect("acting player has a played card"),
        )
    }

    pub(crate) fn name(&self, player: PlayerId) -> &str {
        &self.player(player).hero_name
    }

    pub(crate) fn place(&self, hex: Hex) -> String {
        match self.board.space(hex) {
            Some(s) => format!("{} ({},{})", self.board.zone(s.zone).name, hex.q, hex.r),
            None => format!("({},{})", hex.q, hex.r),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Color;

    fn play_everyone(game: &mut Game, pick: impl Fn(&Game, PlayerId) -> CardId) {
        for i in 0..game.players.len() as u8 {
            let player = PlayerId(i);
            let card = pick(game, player);
            game.apply(Action::PlayCard { player, card }).unwrap();
        }
    }

    fn card_of_color(game: &Game, player: PlayerId, color: Color) -> CardId {
        let p = game.player(player);
        *p.hand
            .iter()
            .find(|c| p.card(&game.catalog, **c).color == color)
            .unwrap()
    }

    #[test]
    fn play_phase_needs_every_player_then_reveals_by_initiative() {
        let mut game = Game::teaching_game();
        assert_eq!(game.phase, Phase::Play);
        play_everyone(&mut game, |g, p| card_of_color(g, p, Color::Gold));
        // Everyone played Gold: highest initiative acts first.
        let player = match &game.phase {
            Phase::Act { player } => *player,
            Phase::ChooseActor { candidates } => candidates[0],
            other => panic!("expected an actor, got {other:?}"),
        };
        let top = game.initiative_of(player);
        for p in &game.players {
            assert!(game.initiative_of(p.id) <= top);
        }
    }

    #[test]
    fn illegal_action_is_rejected() {
        let mut game = Game::teaching_game();
        let err = game
            .apply(Action::Hold {
                player: PlayerId(0),
            })
            .unwrap_err();
        assert!(matches!(err, ActionError::Illegal(_)));
    }

    #[test]
    fn a_hero_can_move_with_a_blue_card_and_the_card_resolves() {
        let mut game = Game::teaching_game();
        play_everyone(&mut game, |g, p| card_of_color(g, p, Color::Blue));
        let Phase::Act { player } = game.phase.clone() else {
            // Blue initiatives may tie within a team; pick the first.
            let Phase::ChooseActor { candidates } = game.phase.clone() else {
                panic!()
            };
            game.apply(Action::ChooseActor {
                player: candidates[0],
            })
            .unwrap();
            return a_hero_moves(&mut game);
        };
        let _ = player;
        a_hero_moves(&mut game);
    }

    fn a_hero_moves(game: &mut Game) {
        let Phase::Act { player } = game.phase.clone() else {
            panic!()
        };
        let from = game.hero_hex(player).unwrap();
        let moves: Vec<Action> = game
            .legal_actions()
            .into_iter()
            .filter(|a| matches!(a, Action::Move { .. }))
            .collect();
        assert!(!moves.is_empty());
        let mv = moves[0].clone();
        game.apply(mv.clone()).unwrap();
        let Action::Move { to, .. } = mv else {
            panic!()
        };
        assert_eq!(game.hero_hex(player), Some(to));
        assert_ne!(from, to);
        assert!(game.player(player).played.is_none());
        assert_eq!(game.player(player).resolved.len(), 1);
    }

    #[test]
    fn four_turns_of_holding_end_the_round_and_cards_come_back() {
        let mut game = Game::teaching_game();
        for turn in 1..=4 {
            assert_eq!(game.turn, turn);
            play_everyone(&mut game, |g, p| g.player(p).hand[0]);
            while !matches!(game.phase, Phase::Play | Phase::LevelUp { .. }) {
                let action = match &game.phase {
                    Phase::Act { player } => Action::Hold { player: *player },
                    Phase::ChooseActor { candidates } => Action::ChooseActor {
                        player: candidates[0],
                    },
                    other => panic!("unexpected {other:?}"),
                };
                game.apply(action).unwrap();
            }
        }
        assert_eq!(game.round, 2);
        assert_eq!(game.turn, 1);
        for p in &game.players {
            assert_eq!(p.hand.len(), 5, "{} got the cards back", p.hero_name);
            assert_eq!(p.coins, 1, "pity coin");
        }
    }
}

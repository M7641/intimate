//! Headless play: a tiny RNG and a random policy, enough to run whole games.

use crate::board::Team;
use crate::game::{Game, Setup};
use crate::hex::Hex;
use crate::turn::Action;

/// xorshift64*. Deterministic, dependency-free, good enough for play-outs.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// How a headless player picks among legal actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Uniform over legal actions. Aimless: games rarely end.
    Random,
    /// Attack when possible, else close in on the nearest enemy, else random.
    Brawler,
}

impl Game {
    /// One legal action under `policy`. `None` when the game is over.
    pub fn pick_action(&self, policy: Policy, rng: &mut Rng) -> Option<Action> {
        let actions = self.legal_actions();
        if actions.is_empty() {
            return None;
        }
        let choice = match policy {
            Policy::Random => actions[rng.below(actions.len())].clone(),
            Policy::Brawler => self.brawl(&actions, rng),
        };
        Some(choice)
    }

    fn brawl(&self, actions: &[Action], rng: &mut Rng) -> Action {
        let attacks: Vec<&Action> = actions
            .iter()
            .filter(|a| matches!(a, Action::Attack { .. }))
            .collect();
        if !attacks.is_empty() {
            return attacks[rng.below(attacks.len())].clone();
        }
        // Defend with the weakest card that holds; take the hit otherwise.
        if let crate::turn::Phase::Defend {
            defender,
            attack_total,
            ..
        } = &self.phase
        {
            let p = self.player(*defender);
            let hex = self.hero_hex(*defender).expect("defender on board");
            let modifiers = self.minion_defense_modifiers(hex, p.team);
            let items = i32::from(p.item_bonus(&self.catalog, crate::card::ItemIcon::Defense));
            let holds = actions.iter().filter_map(|a| match a {
                Action::Defend {
                    card: Some(c),
                    player,
                } => {
                    let v = i32::from(p.card(&self.catalog, *c).defense_value()?);
                    (v + items + modifiers >= i32::from(*attack_total)).then_some((v, *player, *c))
                }
                _ => None,
            });
            return match holds.min_by_key(|(v, _, _)| *v) {
                Some((_, player, card)) => Action::Defend {
                    player,
                    card: Some(card),
                },
                None => Action::Defend {
                    player: *defender,
                    card: None,
                },
            };
        }
        let moves: Vec<(&Action, i32)> = actions
            .iter()
            .filter_map(|a| match a {
                Action::Move { player, to } | Action::FastTravel { player, to } => Some((
                    a,
                    self.distance_to_nearest_enemy(*to, self.player(*player).team),
                )),
                _ => None,
            })
            .collect();
        // Break ties at random: the first in list order would favour one
        // side, since turning the board reverses hex order.
        let best = moves.iter().map(|(_, d)| *d).min();
        let closing: Vec<&Action> = moves
            .iter()
            .filter(|(_, d)| Some(*d) == best)
            .map(|(a, _)| *a)
            .collect();
        if closing.is_empty() {
            actions[rng.below(actions.len())].clone()
        } else {
            closing[rng.below(closing.len())].clone()
        }
    }

    fn distance_to_nearest_enemy(&self, from: Hex, team: Team) -> i32 {
        self.units
            .values()
            .filter(|u| u.team != team)
            .filter_map(|u| u.hex)
            .map(|h| from.distance(h))
            .min()
            .unwrap_or(i32::MAX)
    }

    /// Apply `policy` until the game ends or `max_steps` pass.
    /// Returns the number of actions taken.
    pub fn play_out(&mut self, policy: Policy, rng: &mut Rng, max_steps: usize) -> usize {
        let mut steps = 0;
        while steps < max_steps {
            let Some(action) = self.pick_action(policy, rng) else {
                break;
            };
            self.apply(action).expect("picked action is legal");
            steps += 1;
        }
        steps
    }
}

pub fn play_game(setup: &Setup, policy: Policy, seed: u64, max_steps: usize) -> (Game, usize) {
    let mut game = Game::new(setup).expect("setup is valid");
    let mut rng = Rng::new(seed);
    let steps = game.play_out(policy, &mut rng, max_steps);
    (game, steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn::Phase;
    use std::rc::Rc;

    #[test]
    fn brawler_games_finish() {
        let catalog = Rc::new(crate::card::Catalog::load());
        let setup = Setup::teaching_game();
        for seed in 1..=20 {
            let mut game = Game::with_catalog(&setup, catalog.clone()).unwrap();
            let mut rng = Rng::new(seed);
            let steps = game.play_out(Policy::Brawler, &mut rng, 5_000);
            assert!(
                matches!(game.phase, Phase::GameOver { .. }),
                "seed {seed} still running after {steps} steps in round {}: {:?}",
                game.round,
                game.phase
            );
        }
    }

    #[test]
    fn random_play_never_panics() {
        let catalog = Rc::new(crate::card::Catalog::load());
        let setup = Setup::teaching_game();
        for seed in 1..=5 {
            let mut game = Game::with_catalog(&setup, catalog.clone()).unwrap();
            let mut rng = Rng::new(seed);
            game.play_out(Policy::Random, &mut rng, 3_000);
        }
    }

    #[test]
    fn a_finished_game_offers_no_actions() {
        let (game, _) = play_game(&Setup::teaching_game(), Policy::Brawler, 7, 5_000);
        assert!(game.legal_actions().is_empty());
        assert!(game.is_over());
    }
}

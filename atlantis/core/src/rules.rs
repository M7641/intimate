//! Numbers from the rulebook, in one place.

use crate::board::MinionKind;

pub const TURNS_PER_ROUND: u8 = 4;
pub const MAX_LEVEL: u8 = 8;

/// Coins to go from `level - 1` to `level`. Level 2 costs 1, level 8 costs 7.
pub fn level_up_cost(level: u8) -> u8 {
    level - 1
}

/// Coins the attacker gains for defeating a hero of this level.
pub fn defeat_coins(level: u8) -> u8 {
    level
}

/// Coins each friendly hero of the attacker gains.
pub fn assist_coins(level: u8) -> u8 {
    match level {
        0..=3 => 1,
        4..=6 => 2,
        _ => 3,
    }
}

/// Life counters a team spends when a hero with this lowest tier falls.
pub fn life_cost(lowest_tier: u8) -> u8 {
    lowest_tier
}

pub fn minion_bounty(kind: MinionKind) -> u8 {
    match kind {
        MinionKind::Heavy => 4,
        MinionKind::Melee | MinionKind::Ranged => 2,
    }
}

/// Wave and life counters for a game length and player count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameLength {
    pub waves: u8,
    pub lives: u8,
}

pub fn quick_game(players: usize) -> GameLength {
    GameLength {
        waves: 3,
        lives: if players <= 4 { 4 } else { 5 },
    }
}

pub fn long_game(players: usize) -> GameLength {
    GameLength {
        waves: 5,
        lives: if players <= 4 { 6 } else { 8 },
    }
}

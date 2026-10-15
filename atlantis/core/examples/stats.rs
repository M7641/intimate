//! Play many bot games headless and summarise them.
//!
//!     cargo run -p atlantis-core --release --example stats -- 500

use atlantis_core::sim::{Policy, Rng};
use atlantis_core::{Catalog, Game, Phase, Setup, Team};
use std::collections::BTreeMap;
use std::rc::Rc;

fn main() {
    let games: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let catalog = Rc::new(Catalog::load());
    let setup = Setup::teaching_game();
    let mut wins: BTreeMap<Team, u32> = BTreeMap::new();
    let mut reasons: BTreeMap<String, u32> = BTreeMap::new();
    let mut rounds = Vec::new();
    let mut unfinished = 0;
    let start = std::time::Instant::now();
    for seed in 1..=games {
        let mut game = Game::with_catalog(&setup, catalog.clone()).expect("valid setup");
        let mut rng = Rng::new(seed);
        game.play_out(Policy::Brawler, &mut rng, 10_000);
        match &game.phase {
            Phase::GameOver { winner, reason } => {
                *wins.entry(*winner).or_default() += 1;
                *reasons.entry(reason.clone()).or_default() += 1;
                rounds.push(game.round);
            }
            _ => unfinished += 1,
        }
    }
    let elapsed = start.elapsed();
    rounds.sort_unstable();
    println!(
        "{games} games in {elapsed:.2?} ({:.1} ms/game)",
        elapsed.as_secs_f64() * 1000.0 / games as f64
    );
    println!("unfinished: {unfinished}");
    for (team, n) in &wins {
        println!("{team:?} wins: {n}");
    }
    for (reason, n) in &reasons {
        println!("  {reason}: {n}");
    }
    if !rounds.is_empty() {
        let mid = rounds[rounds.len() / 2];
        println!(
            "rounds: min {} median {mid} max {}",
            rounds[0],
            rounds[rounds.len() - 1]
        );
    }
}

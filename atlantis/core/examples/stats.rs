//! Play many bot games headless and summarise them.
//!
//!     cargo run -p atlantis-core --release --example stats -- 500
//!
//! Options, after the game count:
//!
//!     --red wasp,xargatha    heroes for one team (default: teaching game)
//!     --blue wasp,xargatha
//!     --vary-coin            a new tie-breaker flip every game
//!                            (default: the teaching game's seed in all games)
//!     --shuffle-seats        each game, pick at random which team is seated
//!                            first (default: red)

use atlantis_core::sim::{Policy, Rng};
use atlantis_core::{Catalog, Game, Phase, Setup, Team};
use std::collections::BTreeMap;
use std::rc::Rc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games: u64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(200);
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(|v| v.split(',').map(str::to_string).collect::<Vec<_>>())
    };
    let vary_coin = args.iter().any(|a| a == "--vary-coin");
    let shuffle_seats = args.iter().any(|a| a == "--shuffle-seats");
    let catalog = Rc::new(Catalog::load());
    let mut setup = Setup::teaching_game();
    if let Some(red) = option("--red") {
        setup.red = red;
    }
    if let Some(blue) = option("--blue") {
        setup.blue = blue;
    }
    println!(
        "red {:?} vs blue {:?}, coin {}, seats {}",
        setup.red,
        setup.blue,
        if vary_coin { "varied" } else { "fixed" },
        if shuffle_seats {
            "shuffled"
        } else {
            "red first"
        }
    );
    let mut wins: BTreeMap<Team, u32> = BTreeMap::new();
    let mut reasons: BTreeMap<String, u32> = BTreeMap::new();
    let mut rounds = Vec::new();
    let mut unfinished = 0;
    let start = std::time::Instant::now();
    for seed in 1..=games {
        if vary_coin {
            setup.seed = seed;
        }
        if shuffle_seats {
            // Its own stream, so it does not track the coin's flip.
            setup.seated_first = if Rng::new(seed ^ 0x5EA7).below(2) == 0 {
                Team::Red
            } else {
                Team::Blue
            };
        }
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

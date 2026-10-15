//! Hero profiles from `data/heroes.csv`: display name, complexity, roles,
//! stat profile. The deck lives in the card catalogue under `key`.

use crate::card::{CardDef, Catalog};
use serde::Serialize;

/// A stat as the sheet prints it: "5 (6)" is 5, and 6 at the hero's best.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Rating {
    pub base: u8,
    pub peak: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HeroProfile {
    /// Short name, lower case: the catalogue key ("wasp").
    pub key: String,
    pub name: String,
    pub stars: u8,
    pub set: String,
    pub roles: Vec<String>,
    pub attack: Rating,
    pub initiative: Rating,
    pub defense: Rating,
    pub movement: Rating,
}

/// One hero as a card browser shows it: the profile and the whole deck.
#[derive(Clone, Debug, Serialize)]
pub struct HeroSheet {
    pub profile: HeroProfile,
    pub cards: Vec<CardDef>,
}

const HEROES_CSV: &str = include_str!("../../data/heroes.csv");

pub fn hero_profiles() -> Vec<HeroProfile> {
    HEROES_CSV
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let f = split_csv(line);
            let name = f[0].clone();
            let mut roles = vec![f[3].clone(), f[4].clone()];
            roles.extend(
                f[5].split(',')
                    .map(|r| r.trim().to_string())
                    .filter(|r| !r.is_empty() && r != "-"),
            );
            HeroProfile {
                key: hero_key(&name),
                name,
                stars: f[1].parse().unwrap_or(1),
                set: f[2].clone(),
                roles,
                attack: rating(&f[6]),
                initiative: rating(&f[7]),
                defense: rating(&f[8]),
                movement: rating(&f[9]),
            }
        })
        .collect()
}

pub fn hero_profile(key: &str) -> Option<HeroProfile> {
    hero_profiles().into_iter().find(|h| h.key == key)
}

/// Every hero that has a deck, in sheet order.
pub fn hero_sheets(catalog: &Catalog) -> Vec<HeroSheet> {
    hero_profiles()
        .into_iter()
        .filter_map(|profile| {
            let cards = catalog.deck(&profile.key)?.to_vec();
            Some(HeroSheet { profile, cards })
        })
        .collect()
}

/// "5 (6)" -> base 5, peak 6. "8" -> base 8, no peak.
fn rating(field: &str) -> Rating {
    let mut numbers = field
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse().ok());
    Rating {
        base: numbers.next().unwrap_or(0),
        peak: numbers.next(),
    }
}

/// "Wasp the Warmaiden" -> "wasp". "Widget and Pyro" -> "widget".
pub fn hero_key(name: &str) -> String {
    name.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    fields.push(cur);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_load_with_roles() {
        let heroes = hero_profiles();
        assert_eq!(heroes.len(), 32);
        let arien = hero_profile("arien").unwrap();
        assert_eq!(arien.name, "Arien the Tidemaster");
        assert_eq!(arien.roles, ["Tactician", "Disabler", "Durable", "Pusher"]);
        assert_eq!(arien.stars, 1);
        assert_eq!(
            arien.attack,
            Rating {
                base: 8,
                peak: None
            }
        );
        assert_eq!(
            arien.defense,
            Rating {
                base: 5,
                peak: Some(6)
            }
        );
    }

    #[test]
    fn every_hero_has_a_sheet() {
        let catalog = crate::card::Catalog::load();
        let sheets = hero_sheets(&catalog);
        assert_eq!(sheets.len(), 32);
        assert_eq!(catalog.heroes().count(), 32);
        assert!(sheets.iter().all(|s| s.cards.len() >= 19));
    }

    #[test]
    fn teaching_heroes_have_decks() {
        let catalog = crate::card::Catalog::load();
        for key in ["wasp", "xargatha", "arien", "brogan"] {
            assert!(hero_profile(key).is_some(), "{key} profile");
            assert!(catalog.deck(key).is_some(), "{key} deck");
        }
    }
}

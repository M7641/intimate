//! Action cards, loaded from the vendored catalogue `data/cards.json`
//! (Stats of Atlantis, github.com/QuasiStellar/Stats-of-Atlantis).
//!
//! A card carries numbers the engine applies now (initiative, attack,
//! defense, movement, range) and a text the engine does not apply yet.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct CardId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Color {
    Gold,
    Silver,
    Red,
    Green,
    Blue,
    /// The ultimate. Never in hand; a passive once unlocked.
    Purple,
}

impl Color {
    /// Gold and Silver are basic: no tier, never upgraded.
    pub fn is_basic(self) -> bool {
        matches!(self, Color::Gold | Color::Silver)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    Attack,
    Skill,
    Defense,
    Movement,
    /// Usable as a Skill when played, or as a Defense when discarded.
    DefenseSkill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Modifier {
    Range,
    Area,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ItemIcon {
    Attack,
    Defense,
    Initiative,
    Movement,
    Range,
    Area,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ValueSign {
    /// "4+": the text can raise the value.
    Plus,
    Minus,
    /// "!": a Block defense. Succeeds on its condition, not its value.
    Exclamation,
}

/// Which card of its colour and tier: `first` of `second`. Tiers II and III
/// offer two cards each, so 1 of 2 or 2 of 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Variant {
    pub first: u8,
    pub second: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDef {
    /// Position in the hero's deck. Set on load.
    #[serde(default)]
    pub id: CardId,
    pub name: String,
    /// The catalogue's "description". Written back out as `text`.
    #[serde(rename(deserialize = "description"))]
    pub text: String,
    pub color: Color,
    /// I to IV. `None` for basic cards. The catalogue's "level".
    #[serde(rename(deserialize = "level"))]
    pub tier: Option<u8>,
    /// `None` below tier II.
    pub variant: Option<Variant>,
    /// The weaker Gold used to balance odd player counts.
    #[serde(default)]
    pub handicapped: bool,
    /// An additional basic card that starts the game in the deck, not in hand.
    #[serde(default)]
    pub extra: bool,
    #[serde(default)]
    pub initiative: u8,
    pub primary_action: Option<ActionType>,
    pub primary_value: Option<u8>,
    pub primary_value_sign: Option<ValueSign>,
    pub modifier: Option<Modifier>,
    pub modifier_value: Option<u8>,
    pub modifier_value_sign: Option<ValueSign>,
    pub secondary_attack: Option<u8>,
    pub secondary_movement: Option<u8>,
    pub secondary_defense: Option<u8>,
    pub item: Option<ItemIcon>,
    /// The catalogue's tags for what the text does ("MOVING_UNITS").
    /// Kept as loaded; the engine does not read them yet.
    #[serde(default)]
    pub traits: Vec<String>,
}

impl CardDef {
    pub fn is_basic(&self) -> bool {
        self.color.is_basic()
    }

    pub fn is_ultimate(&self) -> bool {
        self.color == Color::Purple
    }

    pub fn is_block(&self) -> bool {
        self.primary_value_sign == Some(ValueSign::Exclamation)
    }

    /// Attack value an Attack action on this card uses.
    pub fn attack_value(&self) -> Option<u8> {
        match self.primary_action {
            Some(ActionType::Attack) => self.primary_value,
            _ => self.secondary_attack,
        }
    }

    /// Defense value usable when this card is discarded to defend.
    /// Block cards have none: they need their text.
    pub fn defense_value(&self) -> Option<u8> {
        match self.primary_action {
            Some(ActionType::Defense | ActionType::DefenseSkill) => self.primary_value,
            _ => self.secondary_defense,
        }
    }

    /// Spaces a Movement action on this card allows.
    pub fn movement_value(&self) -> Option<u8> {
        match self.primary_action {
            Some(ActionType::Movement) => self.primary_value,
            _ => self.secondary_movement,
        }
    }

    pub fn has_skill(&self) -> bool {
        matches!(
            self.primary_action,
            Some(ActionType::Skill | ActionType::DefenseSkill)
        )
    }

    pub fn is_ranged(&self) -> bool {
        self.modifier == Some(Modifier::Range)
    }

    pub fn range(&self) -> Option<u8> {
        self.is_ranged().then_some(self.modifier_value.unwrap_or(1))
    }
}

/// Every hero's deck, keyed by the hero's short name ("wasp").
#[derive(Clone, Debug)]
pub struct Catalog {
    decks: BTreeMap<String, Vec<CardDef>>,
}

const CARDS_JSON: &str = include_str!("../../data/cards.json");

impl Catalog {
    pub fn load() -> Catalog {
        let mut decks: BTreeMap<String, Vec<CardDef>> =
            serde_json::from_str(CARDS_JSON).expect("bundled card catalogue is valid");
        for deck in decks.values_mut() {
            for (i, card) in deck.iter_mut().enumerate() {
                card.id = CardId(i as u8);
            }
        }
        Catalog { decks }
    }

    pub fn heroes(&self) -> impl Iterator<Item = &str> {
        self.decks.keys().map(String::as_str)
    }

    pub fn deck(&self, hero: &str) -> Option<&[CardDef]> {
        self.decks.get(hero).map(Vec::as_slice)
    }

    pub fn card(&self, hero: &str, id: CardId) -> &CardDef {
        &self.decks[hero][id.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_deck_has_the_standard_shape() {
        let catalog = Catalog::load();
        for hero in catalog.heroes() {
            let deck = catalog.deck(hero).unwrap();
            let count = |f: &dyn Fn(&CardDef) -> bool| deck.iter().filter(|c| f(c)).count();
            assert_eq!(
                count(&|c| c.color == Color::Gold && !c.handicapped && !c.extra),
                1,
                "{hero} gold"
            );
            assert_eq!(
                count(&|c| c.color == Color::Silver && !c.handicapped),
                1,
                "{hero} silver"
            );
            // The handicap card replaces one basic card, usually the Gold.
            assert!(count(&|c| c.handicapped) <= 1, "{hero} handicap");
            assert_eq!(count(&|c| c.is_ultimate()), 1, "{hero} ultimate");
            for color in [Color::Red, Color::Green, Color::Blue] {
                assert_eq!(
                    count(&|c| c.color == color && c.tier == Some(1)),
                    1,
                    "{hero} {color:?} I"
                );
                assert_eq!(
                    count(&|c| c.color == color && c.tier == Some(2)),
                    2,
                    "{hero} {color:?} II"
                );
                assert_eq!(
                    count(&|c| c.color == color && c.tier == Some(3)),
                    2,
                    "{hero} {color:?} III"
                );
            }
            for c in deck
                .iter()
                .filter(|c| c.tier == Some(2) || c.tier == Some(3))
            {
                assert!(c.item.is_some(), "{hero} {} needs an item icon", c.name);
                // The two cards of a colour and tier are variants 1 and 2.
                let twin = deck
                    .iter()
                    .find(|d| d.id != c.id && d.color == c.color && d.tier == c.tier)
                    .unwrap();
                let firsts = [c.variant.unwrap().first, twin.variant.unwrap().first];
                assert!(firsts == [1, 2] || firsts == [2, 1], "{hero} {}", c.name);
            }
        }
    }

    #[test]
    fn card_values_follow_the_primary_action() {
        let catalog = Catalog::load();
        let arien = catalog.deck("arien").unwrap();
        let wave = arien.iter().find(|c| c.name == "Rogue Wave").unwrap();
        assert_eq!(wave.attack_value(), Some(4));
        assert_eq!(wave.range(), Some(2));
        assert_eq!(wave.movement_value(), Some(4));
        assert_eq!(wave.defense_value(), Some(4));
        let duelist = arien.iter().find(|c| c.name == "Aspiring Duelist").unwrap();
        assert_eq!(duelist.defense_value(), Some(5));
        assert_eq!(duelist.attack_value(), None);
        assert_eq!(duelist.movement_value(), Some(3));
    }

    #[test]
    fn a_card_serialises_under_the_engine_names() {
        let catalog = Catalog::load();
        let duelist = &catalog.deck("arien").unwrap()[4];
        let json = serde_json::to_value(duelist).unwrap();
        assert_eq!(json["name"], "Aspiring Duelist");
        assert_eq!(json["tier"], 1);
        assert!(
            json["text"]
                .as_str()
                .unwrap()
                .starts_with("Ignore all minion")
        );
        assert!(json.get("description").is_none());
        assert!(json.get("level").is_none());
    }

    #[test]
    fn a_secondary_attack_gives_an_attack_value() {
        let catalog = Catalog::load();
        let mrak = catalog.deck("mrak").unwrap();
        let slam = mrak.iter().find(|c| c.name == "Seismic Slam").unwrap();
        assert_eq!(slam.primary_action, Some(ActionType::Skill));
        assert_eq!(slam.attack_value(), Some(6));
    }

    #[test]
    fn extra_basic_cards_are_flagged() {
        let catalog = Catalog::load();
        let takahide = catalog.deck("takahide").unwrap();
        let extra: Vec<&str> = takahide
            .iter()
            .filter(|c| c.extra)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(extra, ["Sting like a Bee", "Strike like a Tiger"]);
    }
}

//! A player: one hero, one deck, coins and level.

use crate::board::Team;
use crate::card::{CardDef, CardId, Catalog, ItemIcon};
use crate::unit::UnitId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u8);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: PlayerId,
    pub team: Team,
    pub hero_key: String,
    pub hero_name: String,
    pub hero: UnitId,
    /// Card states, per the rulebook: in hand, played (unresolved),
    /// resolved (on the dashboard this round), discarded, or in the deck.
    pub hand: Vec<CardId>,
    pub played: Option<CardId>,
    pub resolved: Vec<CardId>,
    pub discard: Vec<CardId>,
    pub deck: Vec<CardId>,
    /// Cards tucked under the dashboard as items, and the bonus each gives.
    pub item_cards: Vec<CardId>,
    pub ultimate: Option<CardId>,
    pub coins: u8,
    pub level: u8,
    /// Announced a pass this turn (no cards in hand).
    pub passed: bool,
    pub leveled_this_round: bool,
}

impl Player {
    /// +1 per item whose icon matches.
    pub fn item_bonus(&self, catalog: &Catalog, icon: ItemIcon) -> u8 {
        self.item_cards
            .iter()
            .filter(|c| catalog.card(&self.hero_key, **c).item == Some(icon))
            .count() as u8
    }

    pub fn card<'a>(&self, catalog: &'a Catalog, id: CardId) -> &'a CardDef {
        catalog.card(&self.hero_key, id)
    }

    /// Every action card the player holds outside the deck, whatever its state.
    pub fn owned(&self) -> impl Iterator<Item = CardId> + '_ {
        self.hand
            .iter()
            .chain(self.played.iter())
            .chain(self.resolved.iter())
            .chain(self.discard.iter())
            .copied()
    }

    /// Lowest tier among the non-basic cards held. Drives defeat costs.
    pub fn lowest_tier(&self, catalog: &Catalog) -> u8 {
        self.owned()
            .filter_map(|c| self.card(catalog, c).tier)
            .min()
            .unwrap_or(1)
    }
}

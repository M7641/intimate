//! Rules engine for Guards of Atlantis II.
//!
//! No rendering, no I/O. The browser, a CLI or an AI drive it through
//! [`Game`]: read a [`Snapshot`], pick one of its legal actions, apply it.

pub mod board;
pub mod card;
mod combat;
pub mod game;
pub mod hero;
pub mod hex;
mod movement;
pub mod player;
mod round;
pub mod rules;
pub mod sim;
pub mod turn;
pub mod unit;

pub use board::{Board, MinionKind, Space, SpawnPoint, Team, Zone, ZoneId, ZoneKind};
pub use card::{ActionType, CardDef, CardId, Catalog, Color, ItemIcon};
pub use game::{Game, Setup, SetupError, Snapshot};
pub use hex::Hex;
pub use player::{Player, PlayerId};
pub use turn::{Action, ActionError, Phase};
pub use unit::{Unit, UnitId, UnitKind};

//! Units: heroes and minions. Anything that stands on a hex and can act or
//! be acted upon. Tokens are not units (rulebook, "Unit").

use crate::board::{MinionKind, Team};
use crate::hex::Hex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnitId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UnitKind {
    Hero { name: String },
    Minion { kind: MinionKind },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unit {
    pub id: UnitId,
    pub team: Team,
    pub kind: UnitKind,
    /// Where the unit stands. `None` while a defeated hero waits to respawn.
    pub hex: Option<Hex>,
}

impl Unit {
    pub fn is_hero(&self) -> bool {
        matches!(self.kind, UnitKind::Hero { .. })
    }

    pub fn minion_kind(&self) -> Option<MinionKind> {
        match self.kind {
            UnitKind::Minion { kind } => Some(kind),
            UnitKind::Hero { .. } => None,
        }
    }
}

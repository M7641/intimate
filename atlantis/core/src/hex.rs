//! Axial hex coordinates, pointy-top orientation.
//!
//! `q` runs along the row, `r` down the rows. The third cube coordinate
//! `s = -q - r` is derived when a formula needs it.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Hex {
    pub q: i32,
    pub r: i32,
}

impl Hex {
    /// The six directions, counter-clockwise from "east".
    pub const DIRECTIONS: [Hex; 6] = [
        Hex { q: 1, r: 0 },
        Hex { q: 1, r: -1 },
        Hex { q: 0, r: -1 },
        Hex { q: -1, r: 0 },
        Hex { q: -1, r: 1 },
        Hex { q: 0, r: 1 },
    ];

    pub const ZERO: Hex = Hex { q: 0, r: 0 };

    pub const fn new(q: i32, r: i32) -> Hex {
        Hex { q, r }
    }

    /// Third cube coordinate.
    pub const fn s(self) -> i32 {
        -self.q - self.r
    }

    /// Convert from "odd-r" offset coordinates: odd rows are shoved right by
    /// half a hex. This is how the board file lays out its rows.
    pub const fn from_offset(col: i32, row: i32) -> Hex {
        Hex {
            q: col - (row - (row & 1)) / 2,
            r: row,
        }
    }

    /// Back to "odd-r" offset coordinates: `(col, row)`.
    pub const fn to_offset(self) -> (i32, i32) {
        (self.q + (self.r - (self.r & 1)) / 2, self.r)
    }

    pub fn neighbors(self) -> [Hex; 6] {
        Self::DIRECTIONS.map(|d| self + d)
    }

    pub const fn scaled(self, n: i32) -> Hex {
        Hex {
            q: self.q * n,
            r: self.r * n,
        }
    }

    /// Number of steps between two hexes, obstacles ignored.
    pub fn distance(self, other: Hex) -> i32 {
        let d = self - other;
        (d.q.abs() + d.r.abs() + d.s().abs()) / 2
    }

    pub fn is_adjacent(self, other: Hex) -> bool {
        self.distance(other) == 1
    }

    /// Unit direction from `self` to `other` when both lie on one straight
    /// line. `None` if they do not, or if they are the same hex.
    pub fn direction_to(self, other: Hex) -> Option<Hex> {
        let d = other - self;
        if d == Hex::ZERO {
            return None;
        }
        let on_line = d.q == 0 || d.r == 0 || d.s() == 0;
        if !on_line {
            return None;
        }
        let n = self.distance(other);
        Some(Hex::new(d.q / n, d.r / n))
    }
}

impl Add for Hex {
    type Output = Hex;
    fn add(self, o: Hex) -> Hex {
        Hex::new(self.q + o.q, self.r + o.r)
    }
}

impl Sub for Hex {
    type Output = Hex;
    fn sub(self, o: Hex) -> Hex {
        Hex::new(self.q - o.q, self.r - o.r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_coordinates_sum_to_zero() {
        let h = Hex::new(3, -5);
        assert_eq!(h.q + h.r + h.s(), 0);
    }

    #[test]
    fn neighbors_are_at_distance_one() {
        let h = Hex::new(2, -1);
        for n in h.neighbors() {
            assert_eq!(h.distance(n), 1);
            assert!(h.is_adjacent(n));
        }
    }

    #[test]
    fn distance_is_symmetric_and_matches_steps() {
        let a = Hex::new(0, 0);
        let b = Hex::new(2, 1);
        assert_eq!(a.distance(b), 3);
        assert_eq!(b.distance(a), 3);
        assert_eq!(a.distance(a), 0);
    }

    #[test]
    fn straight_lines_follow_a_direction() {
        let a = Hex::new(0, 0);
        for d in Hex::DIRECTIONS {
            assert_eq!(a.direction_to(d.scaled(3)), Some(d));
        }
        assert_eq!(a.direction_to(Hex::new(2, 1)), None);
        assert_eq!(a.direction_to(a), None);
    }

    #[test]
    fn odd_r_offset_rows_shift_right() {
        // Row 1 is odd: its column 0 sits to the lower-right of row 0's column 0.
        let top = Hex::from_offset(0, 0);
        let below_right = Hex::from_offset(0, 1);
        assert_eq!(top, Hex::new(0, 0));
        assert_eq!(below_right, Hex::new(0, 1));
        assert!(top.is_adjacent(below_right));
        // Row 2 is even again: column 0 is straight below row 0's column 0.
        assert_eq!(Hex::from_offset(0, 2), Hex::new(-1, 2));
        assert_eq!(Hex::from_offset(1, 2), Hex::new(0, 2));
    }

    #[test]
    fn offset_round_trips() {
        for row in -3..4 {
            for col in -3..4 {
                assert_eq!(Hex::from_offset(col, row).to_offset(), (col, row));
            }
        }
    }
}

//! A tiny, dependency-free, **seeded** fake-data generator.
//!
//! Like `faker` in the JS world, but deterministic: the same seed always yields
//! the same stream, so callers get realistic, varied values that are also
//! reproducible. That makes it ideal for hermetic tests (no checked-in fixtures,
//! no flakiness) and for generating sample data in dev tooling.
//!
//! It has no external dependencies (a small SplitMix64 generator), so it adds
//! nothing to a dependent's build graph and works anywhere.
//!
//! ```
//! use faker::Faker;
//!
//! let mut f = Faker::new(42);
//! let name = f.name();        // e.g. "Grace Hopper"
//! let age = f.int_in(18, 80); // within range
//!
//! // Same seed → same stream.
//! assert_eq!(Faker::new(1).name(), Faker::new(1).name());
//! ```

/// First names drawn from computing pioneers — recognizable and varied.
pub const FIRST_NAMES: &[&str] = &[
    "Ada",
    "Alan",
    "Grace",
    "Edsger",
    "Donald",
    "Barbara",
    "Tim",
    "Margaret",
    "Ken",
    "Dennis",
    "Linus",
    "Radia",
    "Leslie",
    "Katherine",
];

/// Last names paired with [`FIRST_NAMES`].
pub const LAST_NAMES: &[&str] = &[
    "Lovelace",
    "Turing",
    "Hopper",
    "Dijkstra",
    "Knuth",
    "Liskov",
    "Berners-Lee",
    "Hamilton",
    "Thompson",
    "Ritchie",
    "Torvalds",
    "Perlman",
    "Lamport",
    "Johnson",
];

/// Domains used by [`Faker::email`].
pub const EMAIL_DOMAINS: &[&str] = &["example.com", "test.org", "sample.net"];

/// A seeded pseudo-random source (SplitMix64) with convenience generators.
///
/// Not cryptographic — just enough spread for fake data. Deterministic by
/// design: construct with a fixed seed for reproducible output.
pub struct Faker {
    state: u64,
}

impl Faker {
    /// Create a generator from a seed. The same seed reproduces the same stream.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The raw SplitMix64 step — the single source of entropy for everything else.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Pick a (copyable) element from a non-empty slice.
    ///
    /// # Panics
    /// Panics if `items` is empty.
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[(self.next_u64() % items.len() as u64) as usize]
    }

    /// An integer in `[low, high]` (inclusive). Requires `low <= high`.
    pub fn int_in(&mut self, low: i64, high: i64) -> i64 {
        let span = (high - low + 1) as u64;
        low + (self.next_u64() % span) as i64
    }

    /// A float in `[0.0, max)`, rounded to two decimals.
    pub fn float_to(&mut self, max: f64) -> f64 {
        let frac = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        ((frac * max) * 100.0).round() / 100.0
    }

    /// A uniformly random boolean.
    pub fn boolean(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }

    /// A first name from [`FIRST_NAMES`].
    pub fn first_name(&mut self) -> &'static str {
        self.pick(FIRST_NAMES)
    }

    /// A last name from [`LAST_NAMES`].
    pub fn last_name(&mut self) -> &'static str {
        self.pick(LAST_NAMES)
    }

    /// A full name, e.g. `"Grace Hopper"`.
    pub fn name(&mut self) -> String {
        format!("{} {}", self.first_name(), self.last_name())
    }

    /// An email derived from a fresh name, e.g. `"grace.hopper@test.org"`.
    pub fn email(&mut self) -> String {
        let first = self.first_name().to_lowercase();
        let last = self.last_name().to_lowercase().replace('-', "");
        let domain = self.pick(EMAIL_DOMAINS);
        format!("{first}.{last}@{domain}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_reproduces_the_stream() {
        let mut a = Faker::new(7);
        let mut b = Faker::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Faker::new(1);
        let mut b = Faker::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn generated_values_stay_in_range() {
        let mut f = Faker::new(1);
        for _ in 0..1000 {
            assert!((18..=80).contains(&f.int_in(18, 80)));
            assert!((0.0..100.0).contains(&f.float_to(100.0)));
        }
    }

    #[test]
    fn names_and_emails_are_well_formed() {
        let mut f = Faker::new(99);
        assert!(f.name().contains(' '));
        let email = f.email();
        assert!(email.contains('@') && email.contains('.'));
    }
}

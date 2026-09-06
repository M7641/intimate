//! Core conversation types, deliberately small for M1.
//!
//! A conversation is just an ordered list of turns. Pedagogical state (estimated
//! level, struggled-with words) lands in M2 as an event log — see README.

use serde::{Deserialize, Serialize};

/// Who spoke a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The human learner.
    Learner,
    /// The bot tutor.
    Tutor,
}

/// One thing said by one party.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: Role,
    pub text: String,
}

impl Turn {
    pub fn learner(text: impl Into<String>) -> Self {
        Self { role: Role::Learner, text: text.into() }
    }

    pub fn tutor(text: impl Into<String>) -> Self {
        Self { role: Role::Tutor, text: text.into() }
    }
}

/// One sense of a word: a part of speech and a plain-text gloss. A word usually
/// has several (e.g. "chat" as a noun, plus figurative senses).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sense {
    /// The grammatical category, in the target language (e.g. "Nom commun").
    pub part_of_speech: String,
    /// The definition text, stripped of markup.
    pub gloss: String,
}

/// A word's definition, collected from a trusted dictionary. Stored alongside the
/// learner's saved words so it is fetched once and then available offline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub word: String,
    /// Human-readable name of the source (e.g. "Wiktionnaire").
    pub source: String,
    /// Link to the source entry, so the learner can read more and we attribute it.
    pub source_url: String,
    pub senses: Vec<Sense>,
}

/// The learner's approximate proficiency, used to pitch input at "i+1".
/// CEFR-flavoured but coarse on purpose — M1 fixes it; M2 will estimate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Beginner,     // A1–A2
    Intermediate, // B1–B2
    Advanced,     // C1–C2
}

impl Default for Level {
    fn default() -> Self {
        Level::Beginner
    }
}

/// The target language being learned. French only for now, but typed so the rest
/// of the system never hard-codes "French".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    French,
}

impl Language {
    /// Endonym — how the language names itself. Used in prompts.
    pub fn endonym(&self) -> &'static str {
        match self {
            Language::French => "français",
        }
    }
}

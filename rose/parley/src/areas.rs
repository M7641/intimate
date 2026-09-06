//! Everyday topic areas, and the "push the learner somewhere new" mechanic.
//!
//! Left alone, a learner (and a chatty LLM) circles the same comfortable topics —
//! fluent at ordering coffee, lost everywhere else. This module breaks that loop:
//! from the words a learner has produced we infer which everyday *areas* they have
//! touched, then steer the tutor toward one they have not. The steering is
//! deterministic (a keyword map, no model) so it is cheap and reviewable; the LLM
//! still does the actual conversational opening. See the skill map and
//! `docs/architecture/learner-state.md`.

use crate::memory::Memories;

/// One everyday area of life a learner should eventually be able to talk about,
/// with a few French stems that signal they have touched it.
struct Area {
    key: &'static str,
    /// How to name it to the tutor, in the target language.
    label: &'static str,
    /// Lowercase, accent-free stems; a learner word *containing* one counts.
    stems: &'static [&'static str],
}

/// The coarse map of everyday domains. Deliberately small and general — breadth of
/// domain, not depth, is what this mechanic buys. Order is the fallback suggestion
/// order when nothing has been touched yet.
const AREAS: &[Area] = &[
    Area { key: "food", label: "la nourriture et les repas", stems: &["mang", "boi", "repas", "restau", "cuisin", "faim", "pain", "cafe"] },
    Area { key: "family", label: "la famille", stems: &["famill", "pere", "mere", "frere", "soeur", "enfant", "parent"] },
    Area { key: "travel", label: "les voyages", stems: &["voyag", "train", "avion", "vacanc", "valise", "hotel", "parti"] },
    Area { key: "work", label: "le travail", stems: &["travail", "boulot", "bureau", "collegu", "patron", "metier", "reunion"] },
    Area { key: "health", label: "la sante", stems: &["sante", "malad", "medecin", "docteur", "corps", "fatigu", "dormi"] },
    Area { key: "hobbies", label: "les loisirs", stems: &["sport", "musiqu", "lir", "film", "jeu", "jou", "hobby", "velo"] },
    Area { key: "home", label: "la maison et le quotidien", stems: &["maison", "appart", "chambr", "menag", "cuisine", "jardin"] },
    Area { key: "future", label: "les projets et l'avenir", stems: &["demain", "proch", "projet", "avenir", "esper", "voudra", "ferai"] },
    Area { key: "opinions", label: "les opinions et les gouts", stems: &["pens", "crois", "aime", "deteste", "prefere", "avis", "trouv"] },
];

/// Strip a French word to a comparable form: lowercase and accent-folded, so a
/// keyword stem matches regardless of accents in the learner's spelling.
fn fold(word: &str) -> String {
    word.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// Very common French words that carry no topical signal. Kept short on purpose —
/// this is vocabulary capture, not a linguistics-grade stopword list.
const STOPWORDS: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "de", "du", "et", "ou", "mais", "je", "tu",
    "il", "elle", "on", "nous", "vous", "ils", "elles", "ne", "pas", "que", "qui", "est",
    "suis", "es", "a", "ai", "as", "ont", "avec", "pour", "dans", "sur", "au", "aux", "ce",
    "cet", "cette", "mon", "ma", "mes", "ton", "ta", "tes", "son", "sa", "ses", "se", "si",
    "en", "y", "me", "te", "the", "and", "i",
];

/// Pull content words out of a learner utterance: lowercase, accent-folded, split
/// on non-letters, drop stopwords and very short tokens. Deterministic — no model.
pub fn content_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphabetic())
        .map(fold)
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Which area keys the learner has already touched, based on their vocabulary.
fn touched(memories: &Memories) -> Vec<&'static str> {
    let words: Vec<String> = memories.vocab.iter().map(|v| fold(&v.word)).collect();
    AREAS
        .iter()
        .filter(|area| {
            area.stems
                .iter()
                .any(|stem| words.iter().any(|w| w.contains(stem)))
        })
        .map(|area| area.key)
        .collect()
}

/// The first everyday area the learner has not yet touched — the direction to
/// steer next. `None` only once they have touched them all.
fn next_untouched(memories: &Memories) -> Option<&'static Area> {
    let seen = touched(memories);
    AREAS.iter().find(|area| !seen.contains(&area.key))
}

/// The human-readable labels of the areas the learner has already practiced.
/// Powers the Progress page.
pub fn practiced_areas(memories: &Memories) -> Vec<String> {
    let seen = touched(memories);
    AREAS
        .iter()
        .filter(|area| seen.contains(&area.key))
        .map(|area| area.label.to_string())
        .collect()
}

/// The label of the next area to steer toward, if any remain untouched.
pub fn suggested_area(memories: &Memories) -> Option<String> {
    next_untouched(memories).map(|area| area.label.to_string())
}

/// A one-line instruction for the tutor: steer gently toward an unexplored area.
/// Returns `None` when every area has been touched (let the conversation roam).
/// This is the string injected into the pedagogy system prompt each turn.
pub fn steer_line(memories: &Memories) -> Option<String> {
    let target = next_untouched(memories)?;
    Some(format!(
        "STEER: The learner has not yet practiced talking about {}. When it feels \
natural, gently open the conversation toward this area — ask a question that \
invites them there — without announcing that you are changing topic.",
        target.label
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Level;
    use crate::memory::DEFAULT_LEARNER;

    #[test]
    fn content_words_drops_stopwords_and_folds_accents() {
        let words = content_words("Je mange une pomme à Paris");
        assert!(words.contains(&"mange".to_string()));
        assert!(words.contains(&"pomme".to_string()));
        assert!(words.contains(&"paris".to_string()));
        assert!(!words.contains(&"je".to_string())); // stopword
        assert!(!words.contains(&"une".to_string())); // stopword
    }

    #[test]
    fn steering_skips_touched_areas() {
        let mut m = Memories::empty(DEFAULT_LEARNER, Level::Beginner);
        // Talk about food → the steer should move past "food".
        m.record_words(content_words("j'ai mange du pain au restaurant"));
        let line = steer_line(&m).expect("some untouched area remains");
        assert!(!line.contains("nourriture"), "should not steer back to food: {line}");
    }

    #[test]
    fn blank_learner_is_steered_to_the_first_area() {
        let m = Memories::empty(DEFAULT_LEARNER, Level::Beginner);
        let line = steer_line(&m).unwrap();
        assert!(line.contains("nourriture")); // first area in the list
    }
}

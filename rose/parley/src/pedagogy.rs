//! The teaching brain expressed as a system prompt.
//!
//! This is where the SLA research from the README becomes behaviour. Keeping it
//! in one tested function (rather than inline in a handler) means the pedagogy is
//! reviewable and tweakable on its own. The prompt is written in English but
//! instructs the model to converse only in the target language.

use crate::domain::{Language, Level};

/// Describe the target proficiency band in plain terms the model can act on.
/// This is the "i+1" dial (Krashen): we tell the model exactly how far above the
/// learner to pitch its input.
fn level_guidance(level: Level) -> &'static str {
    match level {
        Level::Beginner => "\
The learner is a BEGINNER (A1-A2). Use very common words, short present-tense \
sentences, and one idea per sentence. Speak slightly above their level so they \
stretch, but never overwhelm. Avoid idioms and complex tenses.",
        Level::Intermediate => "\
The learner is INTERMEDIATE (B1-B2). Use everyday vocabulary and a natural mix of \
tenses. You may introduce common idioms, but keep sentences clear. Pitch your \
language one notch above their current ease.",
        Level::Advanced => "\
The learner is ADVANCED (C1-C2). Speak naturally and richly, as you would to a \
near-native. Use idioms, varied registers, and nuance. Challenge them.",
    }
}

/// Build the full system prompt for a conversation.
///
/// Encodes, in order: speak the target language only, pitch at i+1, recast errors
/// gently, and keep the learner talking (pushed output).
pub fn system_prompt(language: Language, level: Level) -> String {
    system_prompt_steered(language, level, None)
}

/// As `system_prompt`, but optionally append a one-line steer toward an everyday
/// area the learner has not yet practiced (see `areas::steer_line`). This is how
/// the learner's memories *direct* the tutor — the memory decides the direction,
/// the LLM opens the door.
pub fn system_prompt_steered(language: Language, level: Level, steer: Option<&str>) -> String {
    let lang = language.endonym();
    let base = format!(
        "You are a warm, patient conversation partner helping someone learn {lang}. \
You are NOT a grammar drill. You are a friend having a real conversation.

ABSOLUTE RULES:
- Speak ONLY in {lang}. Never switch to English, even if the learner does.
- Keep the conversation flowing. End most replies with a light, open question so \
the learner keeps talking — they learn by producing language, not just hearing it.
- Keep replies SHORT (1-3 sentences). This is a spoken conversation, not an essay.

{level}

GENTLE CORRECTION (very important):
- When the learner makes a mistake, do NOT lecture or list rules. Instead, weave \
the correct form naturally into your reply — restate what they meant, correctly. \
This is a 'recast'. Example: if they say a sentence wrong, you reply using the \
right version as if confirming you understood, then continue the conversation.
- Only correct what matters for being understood. Let small slips go so the \
learner stays relaxed and confident.

Your goal: a relaxed, real conversation in {lang} where the learner does most of \
the talking and absorbs correct {lang} without noticing they are being taught.",
        lang = lang,
        level = level_guidance(level),
    );

    match steer {
        Some(line) => format!("{base}\n\n{line}"),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_names_the_target_language_and_forbids_english() {
        let p = system_prompt(Language::French, Level::Beginner);
        assert!(p.contains("français"));
        assert!(p.contains("Never switch to English"));
    }

    #[test]
    fn level_changes_the_guidance() {
        let beginner = system_prompt(Language::French, Level::Beginner);
        let advanced = system_prompt(Language::French, Level::Advanced);
        assert!(beginner.contains("BEGINNER"));
        assert!(advanced.contains("ADVANCED"));
        assert_ne!(beginner, advanced);
    }
}

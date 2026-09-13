//! The languages both pages can work in. The code is what MyMemory expects
//! for `langpair` and what we name in the LLM prompt.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lang {
    pub code: &'static str,
    pub name: &'static str,
}

pub const LANGS: &[Lang] = &[
    Lang { code: "en", name: "English" },
    Lang { code: "fr", name: "French" },
    Lang { code: "es", name: "Spanish" },
    Lang { code: "de", name: "German" },
    Lang { code: "it", name: "Italian" },
    Lang { code: "pt", name: "Portuguese" },
    Lang { code: "nl", name: "Dutch" },
    Lang { code: "ja", name: "Japanese" },
    Lang { code: "zh-CN", name: "Chinese" },
];

/// The language after `current` in the list, wrapping around. Enough of a
/// picker for now; a real dropdown is a later problem.
pub fn next(current: Lang) -> Lang {
    let i = LANGS.iter().position(|l| *l == current).unwrap_or(0);
    LANGS[(i + 1) % LANGS.len()]
}

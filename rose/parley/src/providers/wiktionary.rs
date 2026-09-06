//! Dictionary backed by the Wiktionnaire (fr.wiktionary.org) — French definitions.
//!
//! Wikimedia's REST `definition` endpoint is only enabled on the *English*
//! Wiktionary, so it cannot give us French-language definitions. Instead we read
//! the French Wiktionnaire's raw wikitext through the MediaWiki `action=parse` API
//! and extract the definition lines ourselves. This keeps the glosses in French —
//! in keeping with parley's immersion thesis — and comes straight from the
//! authoritative French Wiktionnaire (openly licensed, CC BY-SA; the UI attributes
//! and links back).
//!
//! The `Dictionary` trait is what made this pivot cheap: swapping the source from
//! the English REST endpoint to the French wikitext API touched only this file.
//! See docs/architecture/dictionary.md for the trade-offs (and the TLFi/CNRTL note).

use async_trait::async_trait;
use serde::Deserialize;

use super::{Dictionary, ProviderResult};
use crate::domain::{Definition, Sense};

/// Wikimedia asks every API client to identify itself with a descriptive
/// User-Agent (their policy); an anonymous default UA can be blocked.
const USER_AGENT: &str = "parley-language-tutor/0.1 (local language-learning app)";

/// At most this many senses per word — enough to be useful, not a wall of text.
const MAX_SENSES: usize = 8;

pub struct WiktionaryDictionary {
    client: reqwest::Client,
    base_url: String,
}

impl WiktionaryDictionary {
    /// Build the client. `base_url` is the wiki origin (default
    /// `https://fr.wiktionary.org`), overridable for tests or a mirror.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// The default source, overridable via `PARLEY_WIKTIONARY_URL`.
    pub fn default_base_url() -> String {
        std::env::var("PARLEY_WIKTIONARY_URL")
            .unwrap_or_else(|_| "https://fr.wiktionary.org".to_string())
    }

    /// One lookup, exactly as spelled. Returns a not-found error rather than an
    /// empty definition so `define` can decide whether to retry.
    async fn fetch(&self, word: &str) -> ProviderResult<Definition> {
        let encoded = encode_query(word);
        let url = format!(
            "{}/w/api.php?action=parse&prop=wikitext&format=json&formatversion=2&redirects=1&page={}",
            self.base_url, encoded
        );

        let resp: ParseResponse = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // A missing page comes back as a structured API error, not an HTTP 404.
        if let Some(error) = resp.error {
            anyhow::bail!("« {word} » introuvable dans le Wiktionnaire ({})", error.code);
        }
        let wikitext = resp
            .parse
            .map(|p| p.wikitext)
            .ok_or_else(|| anyhow::anyhow!("réponse inattendue du Wiktionnaire pour « {word} »"))?;

        let senses = parse_french_senses(&wikitext);
        if senses.is_empty() {
            anyhow::bail!("« {word} » n'a pas de définition française exploitable");
        }

        Ok(Definition {
            word: word.to_string(),
            source: "Wiktionnaire".to_string(),
            source_url: format!("{}/wiki/{}", self.base_url, encode_path_segment(word)),
            senses,
        })
    }
}

#[async_trait]
impl Dictionary for WiktionaryDictionary {
    async fn define(&self, word: &str) -> ProviderResult<Definition> {
        let word = word.trim();
        // Try as spelled first (proper nouns are capitalised), then a lowercase
        // fallback, since most French headwords are lowercase.
        match self.fetch(word).await {
            Ok(def) => Ok(def),
            Err(first) => {
                let lower = word.to_lowercase();
                if lower != word {
                    self.fetch(&lower).await
                } else {
                    Err(first)
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct ParseResponse {
    parse: Option<ParseBody>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ParseBody {
    wikitext: String,
}

#[derive(Deserialize)]
struct ApiError {
    code: String,
}

/// Walk the wikitext and pull out the French definition senses.
///
/// The structure we rely on (stable in the Wiktionnaire):
/// - a language section opens with `== {{langue|fr}} ==`; the next `== {{langue|…`
///   ends it (so we ignore Old French, English, etc. on the same page);
/// - a part of speech opens with `=== {{S|nom|fr…}} ===` — the first arg is the POS;
/// - each sense is a line beginning with `#` (but not `#*`/`#:`, which are examples).
fn parse_french_senses(wikitext: &str) -> Vec<Sense> {
    let mut senses = Vec::new();
    let mut in_french = false;
    let mut pos = String::new();

    for raw in wikitext.lines() {
        let line = raw.trim_start();

        if line.starts_with("==") && line.contains("{{langue|") {
            in_french = extract_between(line, "{{langue|", "}}").map(delimit) == Some("fr".into());
            pos.clear();
            continue;
        }
        if !in_french {
            continue;
        }
        if line.starts_with("===") && line.contains("{{S|") {
            if let Some(raw_pos) = extract_between(line, "{{S|", "}}") {
                pos = capitalize(delimit(raw_pos));
            }
            continue;
        }
        if let Some(body) = definition_body(line) {
            let gloss = clean_wikitext(body);
            if !gloss.is_empty() {
                senses.push(Sense { part_of_speech: pos.clone(), gloss });
                if senses.len() >= MAX_SENSES {
                    break;
                }
            }
        }
    }
    senses
}

/// The body of a definition line, or `None` if the line is not a sense (examples
/// `#*` and quotes `#:` are excluded; sub-senses `##` are kept).
fn definition_body(line: &str) -> Option<&str> {
    if !line.starts_with('#') {
        return None;
    }
    let after = line.trim_start_matches('#');
    if after.starts_with('*') || after.starts_with(':') {
        return None;
    }
    Some(after.trim())
}

/// The substring between `start` and the next `end`, if both are present.
fn extract_between<'a>(hay: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let after = &hay[hay.find(start)? + start.len()..];
    let stop = after.find(end)?;
    Some(&after[..stop])
}

/// Take the first `|`-delimited argument of a template body (e.g. `nom|fr|num=1`
/// → `nom`), trimmed.
fn delimit(template_args: &str) -> String {
    template_args.split('|').next().unwrap_or(template_args).trim().to_string()
}

/// Uppercase the first character, leaving the rest untouched ("nom" → "Nom").
fn capitalize(s: String) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => s,
    }
}

/// Turn a wikitext definition into plain French: drop templates and markup, keep
/// the readable words. Deliberately simple — order matters: templates first (they
/// can wrap links), then links, then bold/italic, then whitespace.
fn clean_wikitext(line: &str) -> String {
    let no_templates = remove_templates(line);
    let no_links = replace_links(&no_templates);
    let plain = no_links.replace("'''", "").replace("''", "");
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove `{{…}}` templates, innermost first so nesting is handled.
fn remove_templates(s: &str) -> String {
    let mut t = s.to_string();
    while let Some(end) = t.find("}}") {
        match t[..end].rfind("{{") {
            Some(start) => t.replace_range(start..end + 2, ""),
            None => break, // stray "}}" with no opener — stop
        }
    }
    t
}

/// Replace `[[target|display]]` with `display` and `[[target]]` with `target`.
fn replace_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("[[") {
        out.push_str(&rest[..i]);
        rest = &rest[i + 2..];
        match rest.find("]]") {
            Some(j) => {
                let inner = &rest[..j];
                out.push_str(inner.rsplit('|').next().unwrap_or(inner));
                rest = &rest[j + 2..];
            }
            None => {
                out.push_str("[[");
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Percent-encode a URL path segment (UTF-8), leaving only the unreserved set.
fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Encode a value for use in a query string. Same as a path segment for our inputs
/// (single words / short phrases); spaces become `%20`, which MediaWiki accepts.
fn encode_query(value: &str) -> String {
    encode_path_segment(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_wikitext_strips_templates_links_and_italics() {
        let line = "{{félins|fr}} [[mammifère|Mammifère]] [[carnivore]] ''félin''";
        assert_eq!(clean_wikitext(line), "Mammifère carnivore félin");
    }

    #[test]
    fn encode_path_segment_handles_accents_and_spaces() {
        assert_eq!(encode_path_segment("chat"), "chat");
        assert_eq!(encode_path_segment("été"), "%C3%A9t%C3%A9");
        assert_eq!(encode_path_segment("pomme de terre"), "pomme%20de%20terre");
    }

    /// Hits the live Wiktionnaire — run explicitly with `--ignored` (needs network).
    #[tokio::test]
    #[ignore]
    async fn live_lookup_returns_french_senses() {
        let dict = WiktionaryDictionary::new(WiktionaryDictionary::default_base_url());
        let def = dict.define("chat").await.expect("chat should be found");
        assert_eq!(def.source, "Wiktionnaire");
        assert!(!def.senses.is_empty());
        eprintln!("{} → {}", def.senses[0].part_of_speech, def.senses[0].gloss);
    }

    #[test]
    fn parses_only_french_senses_with_their_pos() {
        let wikitext = "\
== {{langue|fr}} ==
=== {{S|étymologie}} ===
: du latin
=== {{S|nom|fr|num=1}} ===
# {{félins|fr}} [[mammifère|Mammifère]] domestique.
#* ''Le chat dort.''
# Individu mâle de cet animal.
== {{langue|fro}} ==
=== {{S|nom|fro}} ===
# Vieux mot, à ignorer.";
        let senses = parse_french_senses(wikitext);
        assert_eq!(senses.len(), 2);
        assert_eq!(senses[0].part_of_speech, "Nom");
        assert_eq!(senses[0].gloss, "Mammifère domestique.");
        assert_eq!(senses[1].gloss, "Individu mâle de cet animal.");
    }
}

import { createResource, createSignal, For, Show } from "solid-js";
import { Definition, getProgress, listWords, saveWord } from "../api";

// Group a definition's senses by part of speech, preserving order, so the card can
// show "Nom" once with its senses under it rather than repeating the label.
function groupSenses(def: Definition): [string, string[]][] {
  const groups: [string, string[]][] = [];
  for (const sense of def.senses) {
    const last = groups[groups.length - 1];
    if (last && last[0] === sense.part_of_speech) last[1].push(sense.gloss);
    else groups.push([sense.part_of_speech, [sense.gloss]]);
  }
  return groups;
}

/// The Vocabulary page: save a word and read its definition, collected from a
/// trusted French dictionary (the Wiktionnaire). Saved words are the learner's
/// deliberate curation on top of the words parley captures automatically — the
/// seed of future spaced repetition. See docs/architecture/dictionary.md.
export default function Vocabulary() {
  const [words, { refetch }] = createResource(listWords);
  const [progress] = createResource(getProgress);
  const [input, setInput] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function save(word: string) {
    const w = word.trim();
    if (!w || busy()) return;
    setBusy(true);
    setError(null);
    try {
      await saveWord(w);
      setInput("");
      await refetch();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  // Words parley heard the learner say that aren't saved yet — one-tap to save.
  const suggestions = () => {
    const saved = new Set((words() ?? []).map((w) => w.word));
    return (progress()?.top_words ?? []).filter((w) => !saved.has(w.word)).slice(0, 12);
  };

  return (
    <section class="vocabulary">
      <p class="page-lead">
        Enregistre un mot : parley va chercher sa définition dans un dictionnaire
        français de confiance (le Wiktionnaire) et la garde pour toi.
      </p>

      <form
        class="save-form"
        onSubmit={(e) => {
          e.preventDefault();
          save(input());
        }}
      >
        <input
          type="text"
          placeholder="un mot à enregistrer…"
          value={input()}
          onInput={(e) => setInput(e.currentTarget.value)}
          disabled={busy()}
        />
        <button type="submit" disabled={busy() || !input().trim()}>
          {busy() ? "…" : "Enregistrer"}
        </button>
      </form>

      <Show when={error()}>{(msg) => <p class="error">{msg()}</p>}</Show>

      <Show when={suggestions().length > 0}>
        <h3>Mots que tu as dits</h3>
        <ul class="chips">
          <For each={suggestions()}>
            {(w) => (
              <li>
                <button class="chip chip-add" onClick={() => save(w.word)} disabled={busy()}>
                  + {w.word}
                </button>
              </li>
            )}
          </For>
        </ul>
      </Show>

      <h3>Mots enregistrés</h3>
      <Show when={words.error}>
        <p class="error">Impossible de charger tes mots : {String(words.error)}</p>
      </Show>
      <Show
        when={(words() ?? []).length > 0}
        fallback={<p class="hint">Aucun mot enregistré pour l'instant.</p>}
      >
        <div class="word-list">
          <For each={words()}>
            {(w) => (
              <article class="word-card">
                <header class="word-head">
                  <span class="word-title">{w.word}</span>
                  <Show when={w.definition}>
                    {(def) => (
                      <Show when={def().source_url} fallback={<span class="source">{def().source}</span>}>
                        <a class="source" href={def().source_url} target="_blank" rel="noreferrer">
                          {def().source} ↗
                        </a>
                      </Show>
                    )}
                  </Show>
                </header>

                <Show
                  when={w.definition}
                  fallback={
                    <p class="hint">
                      Définition introuvable — le mot est enregistré, réessaie plus tard.
                    </p>
                  }
                >
                  {(def) => (
                    <For each={groupSenses(def())}>
                      {([pos, glosses]) => (
                        <div class="sense-group">
                          <Show when={pos}>
                            <span class="pos">{pos}</span>
                          </Show>
                          <ol class="senses">
                            <For each={glosses}>{(g) => <li>{g}</li>}</For>
                          </ol>
                        </div>
                      )}
                    </For>
                  )}
                </Show>
              </article>
            )}
          </For>
        </div>
      </Show>
    </section>
  );
}

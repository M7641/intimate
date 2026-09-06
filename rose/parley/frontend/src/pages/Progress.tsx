import { createResource, For, Show } from "solid-js";
import { getProgress } from "../api";

// Turn the coarse backend level into a friendly French label.
const LEVEL_LABEL: Record<string, string> = {
  beginner: "Débutant (A1–A2)",
  intermediate: "Intermédiaire (B1–B2)",
  advanced: "Avancé (C1–C2)",
};

/// The learner reflected back to themselves — the only page whose job is to read
/// the learner-state spine. No model runs; this is a pure read of the memory store
/// (local files or MinIO). See docs/architecture/learner-state.md.
export default function Progress() {
  const [data] = createResource(getProgress);

  return (
    <section class="progress">
      <p class="page-lead">
        Ce que parley retient de toi entre les conversations — et vers quoi il va te
        pousser ensuite.
      </p>

      <Show when={data.error}>
        <p class="error">Impossible de charger la progression : {String(data.error)}</p>
      </Show>

      <Show when={data()} fallback={<p class="hint">Chargement…</p>}>
        {(p) => (
          <>
            <div class="stat-row">
              <div class="stat">
                <span class="stat-num">{LEVEL_LABEL[p().level] ?? p().level}</span>
                <span class="stat-label">Niveau estimé</span>
              </div>
              <div class="stat">
                <span class="stat-num">{p().distinct_words}</span>
                <span class="stat-label">Mots produits</span>
              </div>
              <div class="stat">
                <span class="stat-num">{p().practiced_areas.length} / 9</span>
                <span class="stat-label">Domaines abordés</span>
              </div>
            </div>

            <Show when={p().next_area}>
              {(area) => (
                <div class="steer-card">
                  <span class="steer-label">Prochaine direction</span>
                  <p>
                    parley va t'amener en douceur vers <strong>{area()}</strong>.
                  </p>
                </div>
              )}
            </Show>

            <h3>Domaines déjà abordés</h3>
            <Show
              when={p().practiced_areas.length > 0}
              fallback={<p class="hint">Rien encore — commence une conversation.</p>}
            >
              <ul class="chips">
                <For each={p().practiced_areas}>{(a) => <li class="chip">{a}</li>}</For>
              </ul>
            </Show>

            <h3>Tes mots les plus fréquents</h3>
            <Show
              when={p().top_words.length > 0}
              fallback={<p class="hint">Tes mots apparaîtront ici au fil des conversations.</p>}
            >
              <ul class="chips">
                <For each={p().top_words}>
                  {(w) => (
                    <li class="chip">
                      {w.word} <span class="chip-count">{w.count}</span>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          </>
        )}
      </Show>
    </section>
  );
}

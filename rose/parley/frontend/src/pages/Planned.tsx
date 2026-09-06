import { useParams } from "@solidjs/router";
import { Show } from "solid-js";
import { featureById } from "../features";

/// An honest placeholder for a feature we have designed but not yet built. Rather
/// than a blank "coming soon", it shows *why* the feature exists and *what will
/// power it* — the same theory recorded in docs/. The app teaches its own design.
export default function Planned() {
  const params = useParams();
  const feature = () => featureById(params.id);

  return (
    <Show when={feature()} fallback={<p class="error">Page inconnue.</p>}>
      {(f) => (
        <section class="planned">
          <div class="planned-badge">À venir</div>
          <p class="page-lead">{f().rationale}</p>

          <dl class="theory">
            <dt>Compétence entraînée</dt>
            <dd>{f().skill}</dd>
            <dt>Principe pédagogique (SLA)</dt>
            <dd>{f().principle}</dd>
            <dt>Ce qui l'alimente</dt>
            <dd>{f().poweredBy}</dd>
          </dl>

          <p class="theory-note">
            La théorie complète est consignée dans <code>docs/theory/skill-map.md</code>{" "}
            et <code>docs/theory/what-powers-it.md</code>.
          </p>
        </section>
      )}
    </Show>
  );
}

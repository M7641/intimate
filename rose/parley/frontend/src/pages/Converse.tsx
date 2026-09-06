import { createSignal, For, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { audioUrl, converse } from "../api";
import { Recorder } from "../recorder";

type Role = "learner" | "tutor";
interface Turn {
  role: Role;
  text: string;
}

// Where we are in the loop. Drives the button label and disabled state.
type Status = "idle" | "recording" | "thinking";

/// The core mechanic: hold-to-talk conversation. Speaking is pushed output; the
/// tutor's reply is comprehensible input at i+1 with gentle recasts. See the
/// README's six SLA principles.
export default function Converse() {
  // A store (not a signal) so appending a turn only renders the new bubble.
  const [turns, setTurns] = createStore<Turn[]>([]);
  const [status, setStatus] = createSignal<Status>("idle");
  const [error, setError] = createSignal<string | null>(null);

  // Plain module-level state — no reactivity needed; these never drive the UI.
  let session = "";
  const recorder = new Recorder();

  async function onPress() {
    setError(null);

    if (status() === "idle") {
      try {
        await recorder.start();
        setStatus("recording");
      } catch {
        setError("Micro inaccessible — autorise l'accès au microphone.");
      }
      return;
    }

    if (status() === "recording") {
      const clip = await recorder.stop();
      setStatus("thinking");
      try {
        const res = await converse(clip, session);
        session = res.session;
        setTurns(turns.length, { role: "learner", text: res.transcript });
        setTurns(turns.length, { role: "tutor", text: res.reply });
        if (res.audio) new Audio(audioUrl(res.audio)).play();
      } catch (e) {
        setError(String(e));
      } finally {
        setStatus("idle");
      }
    }
  }

  const label = () =>
    status() === "recording" ? "⏹ Arrêter" : status() === "thinking" ? "…" : "🎤 Parler";

  return (
    <section class="converse">
      <p class="tagline">Parle en français. Je t'écoute.</p>

      <div class="transcript">
        <For each={turns}>
          {(turn) => (
            <div class={`bubble ${turn.role}`}>
              <span class="who">{turn.role === "learner" ? "Toi" : "parley"}</span>
              <span class="text">{turn.text}</span>
            </div>
          )}
        </For>
        <Show when={turns.length === 0}>
          <p class="hint">Appuie sur le bouton et dis quelque chose.</p>
        </Show>
      </div>

      <Show when={error()}>{(msg) => <p class="error">{msg()}</p>}</Show>

      <button class={`talk ${status()}`} onClick={onPress} disabled={status() === "thinking"}>
        {label()}
      </button>
    </section>
  );
}

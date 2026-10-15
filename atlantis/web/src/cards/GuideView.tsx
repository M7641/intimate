import { Match, Switch, createResource } from "solid-js";
import { loadGuide } from "./guides";
import { Markdown } from "./Markdown";

/// A hero's community guide, as vendored in docs/guides/.
export function GuideView(props: { hero: string }) {
  const [text] = createResource(() => props.hero, loadGuide);
  return (
    <Switch fallback={<p class="hint">Loading the guide.</p>}>
      <Match when={text.error}>
        <p class="error">guide failed to load: {String(text.error)}</p>
      </Match>
      <Match when={text()}>{(t) => <Markdown text={t()} />}</Match>
    </Switch>
  );
}

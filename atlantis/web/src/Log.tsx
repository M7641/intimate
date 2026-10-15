import { For } from "solid-js";

export function Log(props: { lines: string[]; max?: number }) {
  const tail = () => props.lines.slice(-(props.max ?? 14));
  return (
    <section class="log">
      <h2>Log</h2>
      <ol start={Math.max(1, props.lines.length - tail().length + 1)}>
        <For each={tail()}>{(l) => <li>{l}</li>}</For>
      </ol>
    </section>
  );
}

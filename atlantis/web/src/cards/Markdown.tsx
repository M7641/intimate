import { For, Match, Show, Switch, type JSX } from "solid-js";
import { Dynamic } from "solid-js/web";

// A renderer for the Markdown that scripts/guides-from-bgg writes, and no
// more: headings, paragraphs, "- " lists, links, and * emphasis.
// It builds elements, never HTML strings, so guide text cannot inject markup.

type Block =
  | { kind: "heading"; level: number; text: string }
  | { kind: "list"; items: string[] }
  | { kind: "para"; text: string };

export function blocks(markdown: string): Block[] {
  const out: Block[] = [];
  for (const chunk of markdown.trim().split(/\n{2,}/)) {
    const lines = chunk.split("\n");
    const heading = lines.length === 1 && /^(#{1,6}) (.*)$/.exec(chunk);
    if (heading) out.push({ kind: "heading", level: heading[1].length, text: heading[2] });
    else if (chunk.startsWith("- ")) {
      // "- " opens an item; an indented line continues the one above.
      const items: string[] = [];
      for (const line of lines) {
        if (line.startsWith("- ")) items.push(line.slice(2));
        else items[items.length - 1] += ` ${line.trim()}`;
      }
      const last = out[out.length - 1];
      if (last?.kind === "list") last.items.push(...items);
      else out.push({ kind: "list", items });
    } else out.push({ kind: "para", text: lines.join(" ") });
  }
  return out;
}

type Run = { text: string; bold: boolean; italic: boolean; href?: string };

// An escaped character, a run of stars, a [label](url) link, or a <url>.
const TOKEN = /\\([*#\\])|(\*{1,3})|\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)|<(https?:\/\/[^>\s]+)>/g;

/// Stars toggle: * italic, ** bold, *** both. The script writes them paired.
export function runs(text: string): Run[] {
  const out: Run[] = [];
  let bold = false;
  let italic = false;
  let from = 0;
  const push = (piece: string, href?: string) => piece && out.push({ text: piece, bold, italic, href });
  for (const m of text.matchAll(TOKEN)) {
    push(text.slice(from, m.index));
    from = m.index + m[0].length;
    if (m[1]) push(m[1]);
    else if (m[2]) {
      if (m[2].length !== 2) italic = !italic;
      if (m[2].length !== 1) bold = !bold;
    } else if (m[3]) push(m[3], m[4]);
    else push(m[5], m[5]);
  }
  push(text.slice(from));
  return out;
}

function Inline(props: { text: string }): JSX.Element {
  return (
    <For each={runs(props.text)}>
      {(run) => {
        const body = () => (
          <Show when={run.href} fallback={run.text}>
            <a href={run.href} target="_blank" rel="noreferrer">
              {run.text}
            </a>
          </Show>
        );
        const italic = () => (
          <Show when={run.italic} fallback={body()}>
            <i>{body()}</i>
          </Show>
        );
        return (
          <Show when={run.bold} fallback={italic()}>
            <b>{italic()}</b>
          </Show>
        );
      }}
    </For>
  );
}

export function Markdown(props: { text: string }) {
  return (
    <div class="markdown">
      <For each={blocks(props.text)}>
        {(b) => (
          <Switch>
            <Match when={b.kind === "heading" && b}>
              {/* The page already owns h1 and the hero's h2. */}
              {(h) => (
                <Dynamic component={`h${Math.min(h().level + 2, 6)}`}>
                  <Inline text={h().text} />
                </Dynamic>
              )}
            </Match>
            <Match when={b.kind === "list" && b}>
              {(l) => (
                <ul>
                  <For each={l().items}>
                    {(item) => (
                      <li>
                        <Inline text={item} />
                      </li>
                    )}
                  </For>
                </ul>
              )}
            </Match>
            <Match when={b.kind === "para" && b}>
              {(p) => (
                <p>
                  <Inline text={p().text} />
                </p>
              )}
            </Match>
          </Switch>
        )}
      </For>
    </div>
  );
}

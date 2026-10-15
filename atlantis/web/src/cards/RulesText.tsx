import { For, Match, Switch, type JSX } from "solid-js";
import { COLOR_HEX } from "../CardRow";

// Card text uses the Stats of Atlantis markup, written for a fixed-width
// card face:
//   **bold**      keyword
//   ~(reminder)   reminder text
//   ::icon::      inline icon
//   >>            option in a list, > continues it on the next line
//   ---           divider
// Line breaks are the card's own wrapping, so we rebuild the paragraphs.

type Block = { kind: "para" | "option" | "note" | "divider"; text: string };

/// A label icon opens its own line: "::rune_axe::: +3 Attack".
const LABEL_LINE = /^::\w+:::/;
/// A line that ends its sentence closes the paragraph.
const SENTENCE_END = /[.!?)]\**$/;

export function blocks(text: string): Block[] {
  const out: Block[] = [];
  // Whether the last block still takes the next plain line.
  let open = false;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    const last = out[out.length - 1];
    if (line === "---") out.push({ kind: "divider", text: "" });
    else if (line.startsWith(">>")) out.push({ kind: "option", text: line.slice(2).trim() });
    else if (line.startsWith(">") && last) last.text += ` ${line.slice(1).trim()}`;
    else if (line.startsWith("~")) out.push({ kind: "note", text: line.slice(1).trim() });
    else if (open && last && !LABEL_LINE.test(line)) last.text += ` ${line}`;
    else out.push({ kind: "para", text: line });
    open = line !== "---" && !SENTENCE_END.test(line);
  }
  return out;
}

const ICON_COLOR: Record<string, string> = {
  gold: COLOR_HEX.GOLD,
  silver: COLOR_HEX.SILVER,
  red: COLOR_HEX.RED,
  green: COLOR_HEX.GREEN,
  blue: COLOR_HEX.BLUE,
  purple: COLOR_HEX.PURPLE,
  orange: "#d9822b",
};

/// Most icons sit before the word they illustrate ("::attack_red:: Attack"),
/// so a coloured pip is enough. Runes and tie-breaker sides stand alone and
/// need their name spelled out.
function Icon(props: { name: string }): JSX.Element {
  const parts = props.name.split("_");
  const color = ICON_COLOR[parts[parts.length - 1]] ?? "#888";
  const readable = parts.join(" ");
  if (parts[0] === "tiebreaker") {
    return (
      <b class="icon-label" style={{ color }}>
        tie-breaker {parts[1]}
      </b>
    );
  }
  if (parts[0] === "rune" && parts.length === 2) {
    return <b class="icon-label">{parts[1]} rune</b>;
  }
  return <i class="pip" style={{ background: color }} title={readable} />;
}

const INLINE = /(\*\*.+?\*\*|::\w+::|~\(.*\))/;
/// "::rune_anvil:: rune" would read "Anvil rune rune" once the icon is spelled out.
const RUNE_WORD = /(::rune_[a-z]+::) rune\b/g;

function Inline(props: { text: string }): JSX.Element {
  return (
    <For each={props.text.replace(RUNE_WORD, "$1").split(INLINE).filter(Boolean)}>
      {(part) => (
        <Switch fallback={part}>
          <Match when={part.startsWith("**")}>
            <b>{part.slice(2, -2)}</b>
          </Match>
          <Match when={part.startsWith("::")}>
            <Icon name={part.slice(2, -2)} />
          </Match>
          <Match when={part.startsWith("~(")}>
            <span class="note">{part.slice(1)}</span>
          </Match>
        </Switch>
      )}
    </For>
  );
}

export function RulesText(props: { text: string }) {
  return (
    <div class="rules-text">
      <For each={blocks(props.text)}>
        {(b) => (
          <Switch>
            <Match when={b.kind === "divider"}>
              <hr />
            </Match>
            <Match when={b.kind !== "divider"}>
              <p class={b.kind}>
                <Inline text={b.text} />
              </p>
            </Match>
          </Switch>
        )}
      </For>
    </div>
  );
}

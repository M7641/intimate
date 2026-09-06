import { A } from "@solidjs/router";
import { For, ParentProps } from "solid-js";
import { FEATURES } from "./features";
import "./styles.css";

// Map a feature id to its route. Converse is the home page; Progress has its own
// path; every planned feature shares the /f/:id theory page.
function href(id: string): string {
  if (id === "converse") return "/";
  if (id === "progress") return "/progress";
  if (id === "vocabulary") return "/vocabulary";
  return `/f/${id}`;
}

// Small glyph per lifecycle stage, so the nav honestly shows what is real.
function statusGlyph(status: string): string {
  return status === "live" ? "●" : status === "in-progress" ? "◐" : "○";
}

/// The app shell: brand, the feature nav, and the routed page. The nav is built
/// from the same feature manifest as the docs, so it always reflects the design.
export default function App(props: ParentProps) {
  return (
    <div class="shell">
      <header class="topbar">
        <span class="brand">parley</span>
        <span class="brand-sub">apprendre en parlant</span>
      </header>

      <nav class="nav">
        <For each={FEATURES}>
          {(f) => (
            <A href={href(f.id)} class="nav-link" end={f.id === "converse"}>
              <span class="nav-title">{f.title}</span>
              <span class={`nav-status ${f.status}`} title={f.status}>
                {statusGlyph(f.status)}
              </span>
            </A>
          )}
        </For>
      </nav>

      <main class="content">{props.children}</main>
    </div>
  );
}

# forage

> To forage is to range over unfamiliar ground and bring back what's there.

Point forage at a base URL with **zero prior knowledge of the site**. It crawls,
exercises the interactive features it discovers, watches the API traffic, and
writes a report of what's broken. It's **time-boxed** — give it ten minutes and
it spends them on the widest set of distinct paths it can reach. It is _not_ a
load tester: one browser tab, sequential, polite.

```
base url ─▶ forage ─▶ report.json
            │           report.html
            ├ crawl ────────────┐    + evidence/ screenshots
            ├ exercise controls │
            ├ observe network   │
            └ validate ─────────┘
```

## What it finds

forage drives a real headless Chrome (via the DevTools Protocol), so it sees
exactly what a user's browser sees — XHR/fetch traffic, console errors, uncaught
exceptions, navigation failures. Each problem becomes a **finding**:

| category          | what it means                                            |
| ----------------- | -------------------------------------------------------- |
| `dead_link`       | a followed link returned 4xx/5xx or failed to load       |
| `server_error`    | any request returned 5xx                                 |
| `failed_xhr`      | an XHR/fetch returned non-2xx or failed at the transport |
| `broken_resource` | an image/script/stylesheet/font 404'd                    |
| `malformed_json`  | a response declared JSON but didn't parse                |
| `empty_response`  | a 2xx data endpoint returned `null`/`[]`/`{}`            |
| `soft_error`      | a 200 whose body carries an `error`/`errors` field       |
| `console_error`   | `console.error` during load or an interaction            |
| `js_exception`    | an uncaught exception                                    |
| `control_error`   | exercising a control errored, hung, or blanked the page  |
| `page_error`      | the page rendered an error/alert state                   |
| `page_timeout`    | navigation never settled within the budget               |
| `skipped`         | a control the safety policy declined to touch (info)     |

Findings dedupe by fingerprint, so one broken asset shared across many pages
collapses to a single entry that lists every page it was `seen_on`.

## How it works

- **Global budget, priority frontier.** Where most crawlers bound work per page,
  forage bounds the _whole run_ against one deadline (`budget.rs`). A max-heap
  (`frontier.rs`) mixes two task kinds: visiting a URL and exercising a control.
  A brand-new _route_ outranks exercising a control, which outranks revisiting a
  known path with a different query — so when time runs short, it was spent on
  breadth first.
- **Generic control exercising.** No site knowledge: injected JS
  (`capture/dom.rs`) returns an addressable inventory of every visible control —
  a robust CSS selector, its kind, options, and owning form. `explore/values.rs`
  derives plausible inputs from labels (a search box gets a word lifted from the
  page so results are non-empty; an email field gets an address). Values are set
  the way a real edit would be, firing `input`/`change` so reactive UIs respond.
- **Response bodies.** forage fetches XHR/fetch bodies over CDP
  (`Network.getResponseBody`) right after a page settles — that's the only window
  Chrome still holds them — so it can judge whether a response is _sensible_, not
  just whether it returned 200.
- **Breakage detection.** Each interaction is bracketed: snapshot the page, act,
  wait for idle, then read the network + console deltas and compare state. A
  collapsed body (white-screen) or a new error sentinel flags a `control_error`.

## Layout

```
src/
  main.rs          CLI + run wiring
  budget.rs        global deadline + politeness + per-action cap
  frontier.rs      priority queue + salvaged URL helpers
  scheduler.rs     the core crawl + exercise loop
  explore/         page.rs · controls.rs · values.rs
  capture/         network.rs · console.rs · dom.rs · visual.rs
  validate.rs      the "is this sensible?" judgements
  safety.rs        cautious-by-default destructive-action policy
  findings.rs      finding model + dedupe
  report.rs        report.json · report.html · report.md · crawl.json
  bundle.rs        timestamped run directory
```

## Install & run

forage needs a Chrome/Chromium runtime on the machine.

```sh
cargo install --path .

forage https://example.com                    # ~10 min default budget
forage https://example.com --budget 120       # a quick 2-minute pass
forage https://example.com --dry-run          # discover + classify, act on nothing
RUST_LOG=forage=debug forage https://example.com
```

The run writes a timestamped bundle to `out/<run-id>/`; open `report.html`.
forage exits non-zero if any error-severity finding was recorded, so it's usable
as a CI gate.

### Options

```
--budget <secs>          global time box (default 600)
--out <dir>              bundle root (default out)
--max-pages <n>          cap pages visited (default 500)
--max-depth <n>          max link depth (default 6)
--max-per-route <n>      cap query permutations per path (default 5)
--rate-limit-ms <ms>     min gap between navigations (default 250)
--action-timeout <secs>  per-task hard cap (default 20)
--no-exercise            crawl + observe only
--submit-forms           allow submitting safe (non-destructive) POST forms
--dry-run                discover + classify controls; act on nothing
--headful                show the browser window
```

## Safety & politeness

forage is **cautious by default** — it explores an unknown site without
triggering anything destructive:

- **Same-origin only.** Off-origin links are never visited or exercised. Single
  tab, sequential, with a rate-limited gap between navigations.
- **Destructive denylist.** Controls whose label/name/selector matches words like
  _delete, remove, pay, checkout, logout, confirm, publish, transfer_ are never
  driven — recorded as a `skipped` finding so coverage gaps stay honest.
- **No auth or payment forms.** Forms holding a password, file, or card field are
  never submitted. POST-form submission is gated behind `--submit-forms`; GET
  (search/filter) forms are submitted normally.
- **`--dry-run`** discovers and classifies every control and reports what it
  _would_ exercise, acting on nothing — a safe first pass against a new target.

It follows real `<a href>` links, so it covers multi-page apps well; SPA
button-routing is a known gap (see below).

## Relationship to calque

forage shares the browser / network / crawl foundation of its sibling
[`calque`](../calque), but inverts the intent. Where **calque records** a live
system's behaviour to regenerate it as code, **forage validates** that behaviour
to find what's broken. The plumbing is the same; the question is opposite.

## Next steps

- SPA / client-side-route exploration (follow JS routing, not just `<a href>`).
- Sample more than one option per `<select>` / more than one value per field.
- robots.txt + crawl-delay awareness.
- Auth-aware exploration (a login recipe, then crawl behind the session).
- Re-run suspected findings to separate flakes from real breakage.
- HAR export of the full network trace alongside the report.

# Source: Plotly Dash → React + FastAPI

Dash is the easier source: it's already Python, so callback *bodies* port almost
directly into FastAPI route handlers. The work is structural — splitting the
reactive monolith — not a language translation.

## The mapping

| Dash | Target |
|---|---|
| `app.layout` (the `html.*` / `dcc.*` tree) | **React component tree** |
| `dcc.Input`, `dcc.Dropdown`, `dcc.Slider`, … | **React controlled inputs** (state in React) |
| `@app.callback(Output, Input, State)` | **FastAPI endpoint** — body becomes the handler |
| The callback **wiring** (`Input`/`Output` deps) | **Client fetch** (React Query) keyed on the input state |
| `dcc.Graph(figure=...)` | React chart (Plotly.js `react-plotly.js`, or Recharts/visx) fed by endpoint JSON |
| `dcc.Store` | React client state, or server session state in FastAPI |
| `dash_table.DataTable` | A React table component fed by endpoint rows |
| `prevent_initial_call`, `dash.no_update` | Fetch enable/skip conditions on the client |

## How a callback becomes an endpoint

A Dash callback is already a pure-ish function of its inputs. Lift the body out,
drop the Dash decorator, and expose it:

```python
# BEFORE (Dash) — UI and compute fused
@app.callback(
    Output("revenue-graph", "figure"),
    Input("region", "value"), Input("date-range", "start_date"), Input("date-range", "end_date"),
)
def update_revenue(region, start, end):
    df = load_sales(region, start, end)        # business logic
    fig = px.bar(df, x="month", y="revenue")   # presentation — DON'T move this to the server
    return fig
```

```python
# AFTER (FastAPI) — endpoint returns DATA, not a figure
@router.get("/api/sales/revenue")
def revenue(region: str, start: date, end: date) -> RevenueResponse:
    df = load_sales(region, start, end)        # business logic, moved verbatim
    return RevenueResponse(rows=df.to_dict("records"))   # data only
```

```tsx
// AFTER (React) — owns the chart; fetch keyed on the inputs
const { data } = useQuery(['revenue', region, start, end],
  () => api.revenue({ region, start, end }))
return <Plot data={toPlotly(data?.rows)} layout={...} />
```

**The boundary rule:** the server returns *data*, the client builds the *figure*.
Returning a Plotly `figure` dict from FastAPI is the lazy port — it ships
presentation logic to the backend and locks you to Plotly forever. Move
`load_sales` (business logic); leave `px.bar` (presentation) on the client.

## Dash-specific gotchas

- **Chained callbacks** (output of one is input to the next) become a sequence of
  fetches, or one composite endpoint. Prefer collapsing a tight chain into a single
  endpoint when the intermediate value isn't independently useful — fewer round
  trips, and the reactive chain was an implementation detail, not a contract.
- **Pattern-matching callbacks** (`ALL`, `MATCH`) map to a parameterized endpoint
  plus a list of React components. Capture the pattern in the manifest's `notes`.
- **`State` vs `Input`** — `Input` triggers recompute (→ part of the React Query
  key), `State` is read-but-doesn't-trigger (→ a fetch param that doesn't invalidate
  the cache). Preserve the distinction or you'll over- or under-fetch.
- **Global mutable state** (module-level DataFrames mutated in callbacks) is a
  parity landmine — it makes the source order-dependent. Flag it in the manifest;
  the FastAPI version must make it explicit (a store, a DB, a dependency), and the
  parity grid must replay inputs in the same order until you do.
- **`dcc.Store` with `storage_type='session'/'local'`** is client state — it belongs
  in React, not a FastAPI endpoint. Only `memory` stores shared across users hint at
  server state.

## Parity notes for Dash

Callbacks are directly callable, which makes the oracle easy: import the source
module and call the (un-decorated, or `.__wrapped__`) callback function over the
input grid to capture expected outputs. Diff the *data* the new endpoint returns
against that — never diff figures, which carry presentation noise.

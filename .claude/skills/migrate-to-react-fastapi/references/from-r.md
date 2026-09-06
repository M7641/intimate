# Source: R Shiny → React + FastAPI

Shiny adds a problem Dash doesn't: the compute is in **R**, and FastAPI is Python.
So before the structural split, you must settle a language decision — and it
changes every endpoint, so make it *before* step 5.

## The decision: rewrite vs bridge (settle first)

| | Rewrite R → Python | Keep R behind a service |
|---|---|---|
| How | Translate the reactive bodies into Python in the FastAPI handlers | Expose the R logic via a [plumber](https://www.rplumber.io/) API (or Rserve); FastAPI calls it |
| Best when | Logic is standard data wrangling (dplyr→pandas/polars), the team will own Python | Heavy R-only stats (specialized models, packages with no Python equal), or too much logic to retranslate safely now |
| Parity risk | Higher — translation can drift (factor handling, NA semantics, 1-indexing, recycling) | Lower — same R code runs; only the transport changes |
| End state | Pure Python stack | Polyglot; R service to operate |

A common **hybrid**: bridge the gnarly statistical core, rewrite the
wrangling/glue. Record the choice per manifest item (`notes: "bridge — uses
survival::coxph"` vs `notes: "rewrite — dplyr groupby"`). Don't make it once for
the whole app; make it per compute unit.

## The mapping

| Shiny | Target |
|---|---|
| `ui` / `fluidPage` / `*Output` layout | **React component tree** |
| `sliderInput`, `selectInput`, `textInput`, … | **React controlled inputs** |
| `reactive({...})`, `eventReactive(...)` | The compute → **FastAPI endpoint** (rewritten or bridged) |
| `observe`/`observeEvent` (side effects) | A client action → a `POST` endpoint, or client state update |
| `render*` (`renderPlot`, `renderTable`, …) | Endpoint returns **data**; React builds the plot/table |
| `reactiveVal` / `reactiveValues` | React client state, or persisted/session state in FastAPI |
| `input$x` dependency | **React Query** key entry (recompute trigger) |
| `req()` / `validate()` | Client-side fetch guards + endpoint input validation (Pydantic) |
| modules (`callModule` / `moduleServer`) | A React component + a grouped router — a natural manifest unit |

## How a reactive becomes an endpoint

```r
# BEFORE (Shiny) — reactive compute + render fused, in R
output$revenue_plot <- renderPlot({
  df <- sales %>% filter(region == input$region) %>%
        group_by(month) %>% summarise(revenue = sum(amount))   # business logic
  ggplot(df, aes(month, revenue)) + geom_col()                 # presentation
})
```

```python
# AFTER (FastAPI) — rewrite path: dplyr → polars/pandas, returns DATA
@router.get("/api/sales/revenue")
def revenue(region: str) -> RevenueResponse:
    df = (sales.filter(pl.col("region") == region)
               .group_by("month").agg(pl.col("amount").sum().alias("revenue")))
    return RevenueResponse(rows=df.to_dicts())     # data only; React draws the chart
```

```python
# AFTER (FastAPI) — bridge path: call the existing R via plumber
@router.get("/api/sales/revenue")
def revenue(region: str) -> RevenueResponse:
    rows = plumber_client.get("/revenue", params={"region": region}).json()
    return RevenueResponse(rows=rows)
```

Same boundary rule as Dash: the server returns **data**, React builds the figure.
`ggplot`/`renderPlot` is presentation — it does not move to the backend.

## R→Python translation landmines (parity killers)

These cause silent numeric divergences the parity grid (orchestration.md) is there
to catch — but knowing them up front saves debugging:

- **NA vs NaN vs None** — R's `NA` is typed and propagates differently than pandas
  `NaN`/`None`. `sum(x, na.rm=TRUE)` ≠ a naive pandas `.sum()` unless you drop NAs.
- **Factors** — R factors carry level order that drives grouping and plot ordering;
  pandas categoricals must replicate it or grouped outputs reorder.
- **1-indexing & inclusive ranges** — R is 1-indexed and `a:b` includes `b`; off-by-one
  on slices/sequences is the classic translation bug.
- **Recycling** — R silently recycles short vectors in arithmetic; Python raises or
  broadcasts differently. Vectorized ops that "just worked" in R may need explicit
  alignment.
- **`stringsAsFactors`, default `drop=TRUE`, `merge` vs `join` key semantics** — set
  these explicitly; R's defaults differ from pandas/polars.
- **Floating-point & RNG** — if any logic samples or uses random seeds, R and Python
  RNGs differ; parity on stochastic outputs needs a fixed, comparable seed or a
  distribution-level assertion, not exact equality.

When in doubt, **bridge that unit** rather than risk a subtle rewrite — correctness
first, retranslate later under test cover.

## Parity notes for Shiny

The oracle is harder than Dash because reactives aren't plain functions. Options:

- **Extract the compute** into a standalone R function (no `input$`/`reactive`) and
  call it over the input grid from `Rscript` to capture expected outputs. This is
  also a healthy refactor of the source.
- **Headless drive** the app with `shinytest2` over the grid and capture outputs.
- For **bridged** units, the plumber endpoint *is* the oracle — call it directly.

Capture data, not rendered plots, and diff with numeric tolerance — the landmines
above mean exact float equality will produce false divergences.

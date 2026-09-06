# Example: measuring a Dash app with gauge

Shows gauge pointed at a real [Plotly Dash](https://dash.plotly.com/) app. The
app (`app.py`) is a normal Dash layout plus a `/work` route on the underlying
Flask server that burns CPU and retains memory, so the load driver has something
to bite on. The `Dockerfile` packages it into an image gauge can measure.

## Run

Everything is in `gauge.toml`, so it is one command (build included):

```sh
uv run gauge run --config examples/dash/gauge.toml
```

The config builds the image from `examples/dash/`, polls the Dash index for
readiness, hammers the `/work` route, reads exact cgroup counters throughout, and
writes a report to `examples/dash/report/`.

Dash serves a cached layout at `/` (nearly free), so the config points load at a
single URL — `/work` — rather than at OpenAPI (Dash publishes no spec). Compare
with `examples/fastapi/`, which derives its load from OpenAPI.

## What it teaches

A sample run:

```
       peak memory  556.7 MB
          CPU time  26.021 CPU·s
  mem ▁▂▃▃▄▄▅▅▆▆▇█  74→556 MB
```

The headline: **a framework's baseline can dominate.** The first sample is ~74 MB
— resident before a single request is served, just the cost of importing `dash`

- `plotly` + `flask`. For a fleet of small services, that fixed per-instance
  overhead often matters more for capacity planning than per-request cost. gauge
  makes both visible: the baseline (first sample) and the climb under load (the
  `mem` sparkline).

## Behaviour under a memory cap

Apply a real cgroup limit below what the app wants and watch it get OOM-killed
cleanly — either uncomment `memory` in `gauge.toml`, or override on the command
line:

```sh
uv run gauge run --config examples/dash/gauge.toml --memory 400m
# → peak memory 382.7 MB ... stopped: OOM-killed   (in red)
```

## Notes

- Dash runs on Flask's dev server here — fine for measurement, not production.
  A real deployment behind gunicorn would fork workers; the cgroup accounting
  already sums the whole container, so that just shows up as higher numbers.
- Hitting the Dash index `/` alone is nearly free (cached layout); `/work`
  exists precisely to create a measurable load signature. To measure real
  callback cost, drive `POST /_dash-update-component` with a proper payload.

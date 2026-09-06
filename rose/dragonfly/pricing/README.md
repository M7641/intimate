# Dragonfly Pricing — one instance, many department engines

An extension of the [dragonfly](../README.md) plugin pilot, exploring a
specific architectural question:

> A single-tenant deployment runs **two instances** of the same software for
> two departments, because each department needs slightly different logic.
> Can we collapse that into **one instance** that switches the logic per
> request instead?

Here the logic is **pricing**. Each department's pricing rules live in their
own Rust → WASM plugin. One FastAPI host loads all of them and picks one per
request based on the `X-Department` header, with a safe default.

```
                         ┌─────────────────────────────┐
  POST /price            │  FastAPI host (one process)  │
  X-Department: retail   │                              │
 ─────────────────────►  │   resolve header → engine    │
                         │        │                     │
                         │        ▼                     │
                         │  ┌───────────┐  retail.wasm  │
                         │  │  Extism   │─ wholesale.wasm│
                         │  │  runtime  │─ default.wasm  │
                         │  └───────────┘                │
                         └─────────────────────────────┘
```

## Layout

```
pricing/
├── plugins/                 # Cargo workspace, one crate per engine
│   ├── shared/              #   the host/plugin data contract (reused)
│   ├── default/             #   list price, no adjustments (fallback)
│   ├── retail/              #   +8% markup, volume discounts, VIP
│   └── wholesale/           #   -10% off list, bulk tiers, key accounts
├── api/
│   ├── app.py               # FastAPI host: load plugins, route by header
│   └── requirements.txt
├── build.sh                 # build all plugins -> WASM
└── demo.sh                  # fire one request at every engine
```

## Run it

```bash
# 1. Build the department plugins to WASM
./build.sh

# 2. Install host deps (a venv is recommended)
python3 -m venv .venv && ./.venv/bin/pip install -r api/requirements.txt

# 3. Start the API
./.venv/bin/uvicorn app:app --app-dir api --port 8000

# 4. In another shell, hit every engine with the same request
./demo.sh
```

## What you see

The **same request** — `{"sku":"WIDGET-001","base_price_cents":1000,"quantity":10,"segment":"vip"}` —
priced three different ways:

| `X-Department` | unit | total | why |
|----------------|------|-------|-----|
| `retail`       | 1080 | 9720  | +8% markup, then 5% volume + 5% VIP off |
| `wholesale`    |  900 | 9000  | −10% off list, no bulk tier at qty 10 |
| _unknown_ / _absent_ | 1000 | 10000 | default engine, no adjustments |

The response carries a `routing` block so the decision is observable, and an
unknown department falls back to default with `fell_back_to_default: true`.

## How routing & isolation work

1. The host loads each `.wasm` **once at startup** (instantiation compiles the
   module — too expensive to do per request) and holds the instances.
2. Per request, the `X-Department` header is mapped to an engine; missing or
   unknown headers resolve to `default`.
3. Plugins are loaded with `wasi=False` — no filesystem, network, or clock is
   ever granted. Least privilege is structural, not policy.
4. If a department plugin **errors**, the host catches it and re-prices with
   the default engine (see `genisis.md`: "if the module crashes, the pipeline
   logs the error and continues with a default"). One department's bad logic
   can't take down the other's pricing.

## Adding a department

1. `cargo new --lib plugins/<name>`, add it to the workspace `members`.
2. Depend on `pricing-shared`, implement `price`.
3. Add `"<name>": "pricing_<name>.wasm"` to `ENGINES` in `app.py`.

No host redeploy of *logic* is required — only registration.

## Further reading

- [CRITIQUE.md](CRITIQUE.md) — an honest assessment of whether this is a good
  idea or an architectural curiosity (and why the constraint justifies WASM).
- [ALTERNATIVES.md](ALTERNATIVES.md) — how the *same* problem would look solved
  with embedded **Lua** or with **Firecracker** microVMs, side by side.

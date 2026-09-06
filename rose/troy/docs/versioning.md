# Versioning — the compatibility gate

This is the layer that turns a folder of schemas into a **governed registry**.
A schema validates data at an instant; a registry governs how a contract is
allowed to *change*. Every PR that touches a contract is checked against the
base branch, and a backward-**incompatible** change fails CI.

## What runs

| Surface | Tool | What it compares |
|---------|------|------------------|
| Protobuf (`registry/proto`) | **`buf breaking`** | the `.proto` vs its version on the base branch |
| OpenAPI (`registry/openapi`) | **`oasdiff breaking`** | the spec vs its base-branch version |
| Marts (`registry/marts`) | **`troy.compat`** (in-repo) | the JSON Schema vs its base-branch version |

`buf breaking` and `oasdiff` are off-the-shelf. Raw JSON Schema has no
equivalent, so `python/troy/compat.py` is the custom piece.

One script drives all three — `scripts/check-compat.sh` — and both
`make check-compat` and CI call it, so **local == CI**. It skips `buf`/`oasdiff`
if they are not installed; the marts check always runs (needs only `uv`).

## Run it

```bash
make compat-demo                  # runnable now: marts checker on bundled v1/v2
make check-compat                 # full gate vs origin/main
make check-compat BASE=HEAD~1     # vs a different baseline
```

CI: `.github/workflows/troy-contracts.yml` runs on PRs touching `rose/troy/**`,
installs buf + oasdiff + uv, and gates against `pull_request.base.sha`.

## What counts as "breaking"

The three surfaces share the intuition "don't break existing consumers" but
differ by **contract direction**, which is why they need different tools.

### Protobuf / OpenAPI — RPC & API direction

A consumer calls the service. Breaking = the old client stops working:
removing a field/endpoint, renaming, changing a type, tightening a required
input. `buf breaking` and `oasdiff` encode these rules out of the box. Protobuf
field numbers make this especially crisp — adding a field is always safe,
reusing a number never is.

### Marts — data-contract direction

The schema describes what the producer (dbt) **guarantees** to consumers reading
the table. Breaking = a guarantee is weakened or the promised value set narrows.
`troy.compat` encodes:

| BREAKING | SAFE (additive) |
|----------|-----------------|
| remove a property | add an optional property |
| add a newly-required property | remove from `required`¹ |
| narrow a type union (`["number","null"]` → `["number"]`) | widen a type union |
| remove an enum value | add an enum value |
| raise `minimum` / lower `maximum` | lower `minimum` / raise `maximum` |
| close `additionalProperties` (`true`→`false`) | open it |

¹ still reported as `WARN` — a no-longer-guaranteed field can surprise readers.
`pattern` changes are `WARN`: sub/superset is undecidable statically.

The checker is a deliberate **heuristic over the common keywords**, not a SAT
solver over full JSON Schema. It catches the changes that actually bite data
consumers; exotic constructs (`oneOf`, `$ref` indirection) fall through to
`WARN`-or-ignore and should be reviewed by a human.

## How a baseline is resolved

`buf breaking` reads the old proto from git itself (`--against '.git#ref=…'`).
For OpenAPI and marts the script does `git show <base>:<path>` into a temp file,
then diffs. If a contract is **new** (no version on the base branch) the check
is skipped — there is nothing to break yet.

## The workflow this enables

1. Need a breaking change? Don't sneak it past the gate — **version the object**:
   `customer.proto` → bump `package …v2`; `dim_customer` → publish a `v2` schema
   and keep `v1` until consumers migrate.
2. The gate then sees an *added* `v2` (additive) rather than a *mutated* `v1`
   (breaking), and the contract evolves without surprising anyone.

That discipline — breaking changes become new versions, never silent mutations —
is the whole point of a registry.

## See also

- `scripts/check-compat.sh` — the shared gate
- `python/troy/compat.py` — the marts checker
- `.github/workflows/troy-contracts.yml` — CI wiring
- `docs/protovalidate.md` — value constraints in the proto

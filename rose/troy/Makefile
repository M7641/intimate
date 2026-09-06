# Troy — contract registry prototype. Three boundaries, three demos.
.PHONY: demo demo-python demo-rust proto-gen check-compat compat-demo help

help:
	@echo "make demo          - run every boundary (Python + Rust)"
	@echo "make demo-python   - ingestion + dbt mart validation (uv)"
	@echo "make demo-rust     - internal Protobuf boundary (cargo)"
	@echo "make proto-gen     - production codegen via buf (needs buf installed)"
	@echo "make check-compat  - versioning gate vs baseline (BASE=origin/main)"
	@echo "make compat-demo   - show the marts checker on bundled v1/v2 schemas"

demo: demo-python demo-rust

demo-python:
	cd python && uv run --quiet python -m troy.demo

demo-rust:
	cd rust && cargo run --quiet

# Canonical schema-first codegen: one .proto -> Rust + Python. Requires `buf`.
proto-gen:
	cd registry/proto && buf lint && buf generate

# Versioning gate: proto (buf breaking) + openapi (oasdiff) + marts (troy.compat)
# vs a git baseline. Skips buf/oasdiff if absent; marts check always runs.
BASE ?= origin/main
check-compat:
	bash scripts/check-compat.sh $(BASE)

# Runnable now, no git needed: the marts checker rejecting a breaking v2 and
# accepting a safe v2 against the live schema.
compat-demo:
	@echo "── breaking candidate (expect exit 1) ──"
	-cd python && uv run --quiet python -m troy.compat \
		../registry/marts/dim_customer.schema.json \
		samples/compat/dim_customer.v2_breaking.json --label dim_customer
	@echo "── safe candidate (expect exit 0) ──"
	cd python && uv run --quiet python -m troy.compat \
		../registry/marts/dim_customer.schema.json \
		samples/compat/dim_customer.v2_safe.json --label dim_customer

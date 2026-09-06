import json
from pathlib import Path

import extism

WASM_PATH = (
    Path(__file__).parent.parent
    / "plugin"
    / "target"
    / "wasm32-unknown-unknown"
    / "release"
    / "dragonfly_plugin.wasm"
)


def main():
    manifest = {"wasm": [{"path": str(WASM_PATH)}]}
    plugin = extism.Plugin(manifest)

    with open(Path(__file__).parent / "sample_data.json") as f:
        records = json.load(f)

    print("=== Validation Pass ===\n")
    valid_records = []
    for record in records:
        result = plugin.call("validate", json.dumps(record))
        validation = json.loads(result)
        status = "PASS" if validation["valid"] else "FAIL"
        errors = validation.get("errors", [])
        print(f"  [{status}] {record['product_code']}", end="")
        if errors:
            print(f"  -> {', '.join(errors)}")
        else:
            print()
        if validation["valid"]:
            valid_records.append(record)

    print(f"\n=== Transform Pass ({len(valid_records)} valid records) ===\n")
    for record in valid_records:
        result = plugin.call("transform", json.dumps(record))
        transformed = json.loads(result)
        print(
            f"  {transformed['product_code']}:"
            f" {transformed['quantity']} x {transformed['unit_price_cents']}c"
            f" = {transformed['total_cents']}c {transformed['currency']}"
            f" (region: {transformed['region']})"
        )


if __name__ == "__main__":
    main()

"""End-to-end demo runnable from the command line: ``lagoon`` (or ``uv run lagoon``).

Spins up MiniStack, stages data in S3, provisions a Postgres warehouse, loads
the data, then queries both stores and prints the results.
"""

from __future__ import annotations

from .ministack import MiniStack
from .pipeline import (
    connect_warehouse,
    load_into_warehouse,
    provision_warehouse,
    query_s3,
    query_warehouse,
    stage_to_s3,
)


def main() -> None:
    print("Starting MiniStack…")
    with MiniStack.start() as ministack:
        print(f"  up at {ministack.endpoint_url}")

        print("Staging sample events in S3…")
        keys = stage_to_s3(ministack)
        print(f"  uploaded {len(keys)} objects")

        print("Provisioning Postgres warehouse via RDS…")
        params = provision_warehouse(ministack)
        with connect_warehouse(params) as conn:
            load_into_warehouse(conn)

            print("\nQuery S3 (staged objects):")
            for key in query_s3(ministack):
                print(f"  - {key}")

            print("\nQuery warehouse (total amount per region):")
            for region, total in query_warehouse(conn):
                print(f"  - {region}: {total}")

    print("\nMiniStack stopped. Done.")


if __name__ == "__main__":
    main()

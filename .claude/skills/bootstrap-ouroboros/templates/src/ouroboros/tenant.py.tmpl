import os

import requests
from rich.console import Console
from rich.table import Table


class Tenant:
    """
    https://github.com/nimbus-labs/nimbus-sdk/blob/7ff7058fddb31dde861f047b1c7b6b75de8b41a5/nimbus/resources/tenants.py#L37
    """

    base_url = "https://service.nimbus.example/quota/api/v1"

    def __init__(self):
        self.available_entity_types = [
            "api-deployment",
            "dataBridge-dataCatalog",
            "dataBridge-dataLake",
            "dataBridge-dataWareHouse",
            "feed",
            "webapp",
            "workflow",
            "workspace",
            "pat",
            "sat",
        ]

    def get_instance_options(self, entity_type: str) -> list[dict]:
        if entity_type not in self.available_entity_types:
            raise ValueError(
                f"Invalid entity type: {entity_type}. "
                f"Must be one of: {', '.join(self.available_entity_types)}"
            )

        response = requests.get(
            f"{self.base_url}/settings/tenant-instance-options",
            headers={"Authorization": os.getenv("API_KEY")},
            params={"entityType": entity_type},
            timeout=10,
        )

        if response.status_code != 200:
            msg = f"Error fetching instance options: {response.status_code} - {response.text}"
            raise RuntimeError(msg)

        return response.json().get("data", [])

    def print_instance_options(self, entity_type: str) -> None:
        console = Console()
        instances = self.get_instance_options(entity_type)

        if not instances:
            console.print(f"[dim]No instance options found for {entity_type}[/dim]")
            return

        grouped: dict[str, list[dict]] = {}
        for inst in instances:
            cls = inst.get("instanceClass", "Unknown")
            grouped.setdefault(cls, []).append(inst)

        for cls in grouped:
            grouped[cls].sort(key=lambda x: x.get("cpu", 0) or 0)

        has_gpu = any(inst.get("gpu") for inst in instances)

        for cls, group in grouped.items():
            table = Table(
                title=f"  {cls}",
                title_style="bold cyan",
                border_style="dim",
                show_header=True,
                header_style="bold magenta",
                padding=(0, 1),
            )

            table.add_column("ID", style="dim", justify="right")
            table.add_column("Name", style="white", min_width=12)
            table.add_column("CPU", style="green", justify="right")
            table.add_column("Memory", style="blue", justify="right")

            if has_gpu:
                table.add_column("GPU", style="yellow", justify="right")
                table.add_column("GPU Mem", style="yellow", justify="right")

            table.add_column("Daily Cost*", style="bold yellow", justify="right")
            table.add_column("Provider", style="dim")

            for inst in group:
                cpu_val = inst.get("cpu", 0) or 0
                mem_val = inst.get("memory", 0) or 0

                cpu = f"{cpu_val / 1000:.3g} vCPU" if cpu_val else "—"
                mem = f"{mem_val / 1000:.3g} GB" if mem_val else "—"

                vcpu = cpu_val / 1000
                gb = mem_val / 1000
                daily_cost = 24 * (vcpu * 0.04 + gb * 0.005)
                cost = f"${daily_cost:.2f}" if daily_cost else "—"

                name = inst.get("name", "").split("(")[0].strip()

                row = [str(inst.get("id", "")), name, cpu, mem]

                if has_gpu:
                    gpu = str(inst["gpu"]) if inst.get("gpu") else "—"
                    gpu_mem = (
                        f"{inst['gpuMemory']} GB" if inst.get("gpuMemory") else "—"
                    )
                    row.extend([gpu, gpu_mem])

                row.append(cost)
                row.append(inst.get("provider", "—"))
                table.add_row(*row)

            console.print()
            console.print(table)

        console.print()
        console.print(
            f"[dim italic]  {len(instances)} instance types available for "
            f"[bold]{entity_type}[/bold][/dim italic]"
        )
        console.print(
            "[dim italic]  *Estimated daily cost based on Fargate pricing "
            "($0.04/vCPU/hr + $0.005/GB/hr, 24h)[/dim italic]"
        )
        console.print()

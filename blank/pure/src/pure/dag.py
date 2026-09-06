import asyncio
import time
from collections.abc import Callable

import networkx as nx

from pure.logging import NimbusLogger

logger = NimbusLogger.get_logger(__name__)


class DAGRunner:
    """
    Lightweight DAG-based task runner built on NetworkX.

    Usage:
        dag = DAGRunner("my_pipeline")
        dag.add_node("load_data", load_data_fn)
        dag.add_node("transform", transform_fn, depends_on=["load_data"])
        dag.add_node("save", save_fn, depends_on=["transform"])
        dag.run()           # sequential
        # or: asyncio.run(dag.run_async())  # concurrent where possible
    """

    def __init__(self, name: str = "pipeline") -> None:
        self.name = name
        self.graph = nx.DiGraph()

    def add_node(
        self,
        name: str,
        fn: Callable,
        depends_on: list[str] | None = None,
    ) -> None:
        self.graph.add_node(name, fn=fn)
        for dep in depends_on or []:
            if dep not in self.graph:
                raise ValueError(
                    f"Dependency '{dep}' not found. Add it before '{name}'."
                )
            self.graph.add_edge(dep, name)

    def run(self) -> None:
        """Execute all nodes sequentially in topological order."""
        if not nx.is_directed_acyclic_graph(self.graph):
            raise ValueError(f"Pipeline '{self.name}' contains a cycle")

        logger.info("Pipeline '%s' started (%d steps)", self.name, len(self.graph))
        t_start = time.monotonic()

        for node_name in nx.topological_sort(self.graph):
            fn = self.graph.nodes[node_name]["fn"]
            logger.info("Starting: %s", node_name)
            t0 = time.monotonic()
            fn()
            logger.info("Completed: %s (%.1fs)", node_name, time.monotonic() - t0)

        logger.info(
            "Pipeline '%s' completed in %.1fs", self.name, time.monotonic() - t_start
        )

    async def run_async(self) -> None:
        """
        Execute the DAG with maximum concurrency for independent nodes.

        Nodes whose dependencies are all satisfied run concurrently.
        Sync callables are offloaded to a thread via asyncio.to_thread().
        Async callables are awaited directly.
        """
        if not nx.is_directed_acyclic_graph(self.graph):
            raise ValueError(f"Pipeline '{self.name}' contains a cycle")

        logger.info(
            "Pipeline '%s' started async (%d steps)", self.name, len(self.graph)
        )
        t_start = time.monotonic()
        completed: set[str] = set()
        all_nodes = set(self.graph.nodes)

        while completed != all_nodes:
            ready = [
                n
                for n in all_nodes - completed
                if all(pred in completed for pred in self.graph.predecessors(n))
            ]

            if not ready:
                raise RuntimeError(
                    "Deadlock detected — no ready nodes but DAG incomplete"
                )

            async def _run_node(name: str) -> str:
                fn = self.graph.nodes[name]["fn"]
                logger.info("Starting: %s", name)
                t0 = time.monotonic()
                if asyncio.iscoroutinefunction(fn):
                    await fn()
                else:
                    await asyncio.to_thread(fn)
                logger.info("Completed: %s (%.1fs)", name, time.monotonic() - t0)
                return name

            results = await asyncio.gather(*[_run_node(n) for n in ready])
            completed.update(results)

        logger.info(
            "Pipeline '%s' completed in %.1fs", self.name, time.monotonic() - t_start
        )

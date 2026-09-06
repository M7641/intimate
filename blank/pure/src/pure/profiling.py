import linecache
import time
import tracemalloc
from typing import Any, Callable

from pure.logging import NimbusLogger

logger = NimbusLogger(__name__).logger


def display_top(
    snapshot: tracemalloc.Snapshot,
    key_type: str = "lineno",
    limit: int = 10,
) -> None:
    snapshot = snapshot.filter_traces(
        (
            tracemalloc.Filter(False, "<frozen importlib._bootstrap>"),
            tracemalloc.Filter(False, "<unknown>"),
        )
    )
    top_stats = snapshot.statistics(key_type)

    logger.info("Top %d lines", limit)
    for index, stat in enumerate(top_stats[:limit], 1):
        frame = stat.traceback[0]
        logger.info(
            "#%d: %s:%d: %.1f KiB",
            index,
            frame.filename,
            frame.lineno,
            stat.size / 1024,
        )
        line = linecache.getline(frame.filename, frame.lineno).strip()
        if line:
            logger.info("    %s", line)

    other = top_stats[limit:]
    if other:
        size = sum(stat.size for stat in other)
        logger.info("%d other: %.1f KiB", len(other), size / 1024)
    total = sum(stat.size for stat in top_stats)
    logger.info("Total allocated size: %.1f KiB", total / 1024)


def profile(func: Callable) -> Callable:
    """
    A decorator that profiles the memory usage of a function.
    """

    def wrapper(*args: Any, **kwargs: Any) -> Any:
        tracemalloc.start()

        result = func(*args, **kwargs)

        snapshot = tracemalloc.take_snapshot()
        display_top(snapshot)

        return result

    return wrapper


def time_this(func: Callable) -> Callable:
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        start = time.time()
        result = func(*args, **kwargs)
        duration = round(time.time() - start, 2)
        logger.info("Function '%s' took %s seconds to execute", func.__name__, duration)
        return result

    return wrapper

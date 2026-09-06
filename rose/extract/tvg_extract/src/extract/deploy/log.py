"""Self-contained coloured logging for ouroboros.

Vendored here so the deploy client carries no external logging dependency.
``NimbusLogger(name).logger`` returns a stdlib ``logging.Logger`` with a coloured
stream handler at INFO level and ``propagate=False`` (no duplicate lines, and it
stays quiet under pytest capture).
"""

from __future__ import annotations

import logging
from typing import ClassVar


class NimbusFormatter(logging.Formatter):
    grey = "\x1b[38;20m"
    blue = "\x1b[34;1m"
    yellow = "\x1b[33;20m"
    red = "\x1b[31;20m"
    bold_red = "\x1b[31;1m"
    reset = "\x1b[0m"
    format_string = (
        "%(asctime)s - %(name)s - %(funcName)20s() - %(levelname)s - %(message)s "
        "(%(filename)s:%(lineno)d)"
    )

    FORMATS: ClassVar[dict[int, str]] = {
        logging.DEBUG: grey + format_string + reset,
        logging.INFO: blue + format_string + reset,
        logging.WARNING: yellow + format_string + reset,
        logging.ERROR: red + format_string + reset,
        logging.CRITICAL: bold_red + format_string + reset,
    }

    def format(self, record: logging.LogRecord) -> str:
        log_fmt = self.FORMATS.get(record.levelno)
        formatter = logging.Formatter(log_fmt)
        return formatter.format(record)


class NimbusLogger:
    def __init__(self, name: str, level: int | str = logging.INFO) -> None:
        logger = logging.getLogger(name)

        # Clear existing handlers to prevent duplicates.
        if logger.handlers:
            logger.handlers.clear()

        try:
            logger.setLevel(level)
        except ValueError:
            logger.setLevel(logging.WARNING)

        stream_handler = logging.StreamHandler()
        stream_handler.setFormatter(NimbusFormatter())
        logger.addHandler(stream_handler)

        # Prevents duplicate logs and keeps logs out of pytest capture.
        logger.propagate = False

        self._logger = logger

    @property
    def logger(self) -> logging.Logger:
        return self._logger

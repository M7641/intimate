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
    """
    Future:
    - Add support for logging to a file with rotation.
    - Add support for logging to a remote server or service.
    - Align with Loki logging standards.
    """
    def __init__(
        self,
        name: str,
        level: int | str = logging.INFO,
    ) -> None:
        logger = logging.getLogger(name)

        # Clear existing handlers to prevent duplicates
        if logger.handlers:
            logger.handlers.clear()

        try:
            logger.setLevel(level)
        except ValueError:
            logger.setLevel(logging.WARNING)

        stream_handler = logging.StreamHandler()
        stream_handler.setFormatter(NimbusFormatter())
        logger.addHandler(stream_handler)

        # This does prevent duplicate logs
        # Also prevents logs from being captured in testing.
        logger.propagate = False

        self._logger = logger

    @property
    def logger(self) -> logging.Logger:
        return self._logger

    @classmethod
    def get_logger(
        cls,
        name: str,
        level: int | str = logging.INFO
    ) -> logging.Logger:
        """
        Get a logger instance with the specified name and level.
        If the logger already exists, it will return the existing instance.
        """
        if name not in logging.Logger.manager.loggerDict:
            return cls(name, level).logger
        else:
            return logging.getLogger(name)


class LogHandler:
    """
    Class to handle recording validation history in a log file.

    I will need to identify what the consistent way is to record these in a
    way where we can actually analyse the results.

    S3, redshift? There won't be loads of data to track so I think
    I can go for something more intense such as redshift just for ease of use.

    If we want to try something like https://github.com/SigNoz/signoz?tab=readme-ov-file
    then that is one way of doing it also.
    """

    def __init__(
            self,
            logger: logging.Logger,
            log_file: str = "validation.log",
        ):
        self.log_file = log_file
        self.logger = logger

    def send_message(self, message: str) -> None:
        """Record a message in the log file."""
        with open(self.log_file, "a") as f:
            f.write(f"{message}\n")
        self.logger.info(f"Logged message: {message}")

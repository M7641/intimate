import os
import time
import psycopg
from pure.logging import NimbusLogger
from database.types import Connector, Connection

logger = NimbusLogger(name=__name__).logger


class RedshiftConnector(Connector):
    """
    This might be leading to queries hanging.
    I think we want to first add some base settings to timeout and to
    look into the redshift_connector docs for best practices around connection management.
    """

    _MAX_ATTEMPTS = 5
    _RETRY_DELAY = 0.5

    def __init__(self) -> None:
        self.config = {
            "database": os.getenv("TENANT"),
            "user": os.getenv("REDSHIFT_USERNAME"),
            "password": os.getenv("REDSHIFT_PASSWORD"),
            "host": os.getenv("REDSHIFT_HOST"),
            "port": 5439,
        }
        self._con_string = f"postgresql://{self.config['user']}:{self.config['password']}@{self.config['host']}:{self.config['port']}/{self.config['database']}?client_encoding=utf-8"
        self._con = self._connect_with_retry()

    def _connect_with_retry(self) -> Connection:
        last_exc: psycopg.OperationalError | None = None
        for attempt in range(1, self._MAX_ATTEMPTS + 1):
            try:
                return psycopg.connect(
                    self._con_string,
                    connect_timeout=30,
                    keepalives=1,
                    keepalives_idle=30,
                    keepalives_interval=10,
                    keepalives_count=5,
                    sslmode='require',
                )
            except psycopg.OperationalError as exc:
                last_exc = exc
                if attempt == self._MAX_ATTEMPTS:
                    break
                logger.warning(
                    "Redshift connection attempt %d/%d failed (%s); retrying in %.2fs",
                    attempt, self._MAX_ATTEMPTS, exc, self._RETRY_DELAY,
                )
                time.sleep(self._RETRY_DELAY)
        assert last_exc is not None
        raise last_exc

    def connection(self) -> Connection:
        return self._con

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        if hasattr(self, '_con'):
            self._con.close()

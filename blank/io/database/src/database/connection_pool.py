from generic_connection_pool.threading import BaseConnectionManager, ConnectionPool
from pure.logging import NimbusLogger

from database.connectors.connector import get_connector
from database.types import Connection

logger = NimbusLogger(name=__name__).logger


class DbConnectionManager(BaseConnectionManager[str, Connection]):
    """
    https://github.com/dapper91/generic-connection-pool/tree/master

    It might be worth extracting the code so we can remove the depdency.
    """

    def __init__(self, db_type: str) -> None:
        self.db_type = db_type

    def create(self, endpoint: str, timeout: float | None = None) -> Connection:
        return get_connector(self.db_type).connection()

    def dispose(
        self, endpoint: str, conn: Connection, timeout: float | None = None
    ) -> None:
        conn.close()

    def check_aliveness(
        self, endpoint: str, conn: Connection, timeout: float | None = None
    ) -> bool:
        try:
            with conn.cursor() as cur:
                cur.execute("SELECT 1;")
                cur.fetchone()
        except (OSError, Exception):
            return False

        return True

    def on_acquire(self, endpoint: str, conn: Connection) -> None:
        self._rollback_uncommitted(conn)

    def on_release(self, endpoint: str, conn: Connection) -> None:
        self._rollback_uncommitted(conn)

    def _rollback_uncommitted(self, conn: Connection) -> None:
        try:
            conn.rollback()
        except Exception:
            pass


class ConnectionPool2(ConnectionPool[str, Connection]):
    def connection(self, timeout: float | None = None) -> Connection:
        logger.debug(
            "Acquired connection from pool. There are now %d connections in use.",
            self.get_size(),
        )
        return super().connection(endpoint="default")


def get_connection_pool(
    db_type: str,
    max_size: int = 10,
) -> ConnectionPool2:
    """
    Get a connection pool for the specified database type.

    This should be good to test in an API now. You do have to close
    this manually otherwise the program will not exit and connections
    will remain open.

    The original idea was not going to work as I was starting a pool
    implicitly and just never had control over it.

    Args:
        db_type (str | None): The type of the database.
        min_size (int): Minimum number of connections in the pool.
        max_size (int): Maximum number of connections in the pool.

    Returns:
        ConnectionPool: A connection pool instance.
    """
    pool = ConnectionPool2(
        connection_manager=DbConnectionManager(db_type=db_type),
        acquire_timeout=2.0,
        idle_timeout=60.0,
        max_lifetime=600.0,
        min_idle=1,
        max_size=max_size,
        total_max_size=15,
        background_collector=True,
    )
    return pool

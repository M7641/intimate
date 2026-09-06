from pathlib import Path
from database.types import Connector, Connection
import libsql

class LibsqlConnector(Connector):
    """
    In memory OTAP database connector using Libsql.
    """
    def __init__(
        self: "LibsqlConnector",
        database_url: str | Path = Path.cwd() / 'datadb' / 'libsql_db_data.db'
    ) -> None:

        if isinstance(database_url, Path):
            database_url.parent.mkdir(parents=True, exist_ok=True)
            database_url = str(database_url)

        self._database_url = str(database_url)
        self._con = libsql.connect(self._database_url)

    def database_url(self: "LibsqlConnector") -> str:
        return self._database_url

    def connection(self: "LibsqlConnector") -> Connection:
        return self._con

    def __enter__(self: "LibsqlConnector") -> "LibsqlConnector":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        if hasattr(self, '_con'):
            self._con.close()

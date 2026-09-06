from pathlib import Path
import duckdb
from database.types import Connector, Connection

class DuckDBConnector(Connector):
    def __init__(
            self: "DuckDBConnector",
            database_url: str | Path = Path.cwd() / 'datadb' / 'duck_db_data.duckdb'
        ) -> None:

        if isinstance(database_url, Path):
            database_url.parent.mkdir(parents=True, exist_ok=True)
            database_url = str(database_url)

        self._database_url = str(database_url)
        self._con = duckdb.connect(database=database_url)

    def database_url(self: "DuckDBConnector") -> str:
        return self._database_url

    def connection(self: "DuckDBConnector") -> Connection:
        return self._con

    def __enter__(self: "DuckDBConnector") -> "DuckDBConnector":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        if hasattr(self, '_con'):
            self._con.close()

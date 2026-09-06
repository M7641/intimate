import os
import pymssql

from database.types import Connector, Connection


class MSSQLConnector(Connector):
    def __init__(self, debug: bool = False) -> None:
        self._con = pymssql.connect(
            user=os.getenv("SQLSERVER_USER", ""),
            password=os.getenv("SQLSERVER_PASSWORD", ""),
            server=os.getenv("SQLSERVER_HOST", ""),
            port=1441,
            database="Nimbus",
            charset="UTF-8",
            timeout=30,
            login_timeout=60,
        )
        self._con._conn.debug_queries = debug

    def connection(self) -> Connection:
        return self._con

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if hasattr(self, '_con') and self._con:
            self._con.close()

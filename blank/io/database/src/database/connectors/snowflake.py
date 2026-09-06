import json
import os

import requests
import snowflake.connector

from database.types import Connection, Connector


class SnowflakeConnector(Connector):
    def __init__(self: "SnowflakeConnector") -> None:
        if os.getenv("SNOWFLAKE_AUTH_TYPE", "OTHER").lower() == "oauth":
            self.init_oauth()
        else:
            self.init_other()

    def init_oauth(self: "SnowflakeConnector") -> None:
        credentials: dict[str, str] = json.loads(
            requests.get(
                "https://service.nimbus.example/connections/api/v1/connections/credentials",
                headers={"Authorization": os.getenv("API_KEY")},
                timeout=10,
            ).content.decode(),
        )
        self._con = snowflake.connector.connect(
            user=credentials.get("user", ""),
            account=credentials.get("host", ""),
            database=credentials.get("database", ""),
            authenticator="oauth",
            schema=credentials.get("schema", ""),
            warehouse=credentials.get("warehouse", ""),
            token=credentials.get("accessToken", ""),
        )

    def init_other(self: "SnowflakeConnector") -> None:
        self.config = {
            "account": os.getenv("SNOWFLAKE_ACCOUNT"),
            "username": os.getenv("SNOWFLAKE_USERNAME"),
            "password": os.getenv("SNOWFLAKE_PASSWORD"),
            "warehouse": os.getenv("SNOWFLAKE_WAREHOUSE"),
            "database": os.getenv("SNOWFLAKE_DATABASE"),
            "schema": os.getenv("SNOWFLAKE_SCHEMA"),
        }
        self._con = snowflake.connector.connect(
            user=self.config["username"],
            password=self.config["password"],
            account=self.config["account"],
            warehouse=self.config["warehouse"],
            database=self.config["database"],
            schema=self.config["schema"],
        )

    def connection(self) -> Connection:
        return self._con

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        if hasattr(self, "_con"):
            self._con.close()

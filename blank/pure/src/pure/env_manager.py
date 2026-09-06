import os

from pure.logging import NimbusLogger

logger = NimbusLogger(name=__name__).logger


class EnvManager:
    def __init__(self: "EnvManager") -> None:
        os.environ["TF_CPP_MIN_LOG_LEVEL"] = "3"

        self.check_key_in_env(["TARGET_ENV", "TENANT"])

        self.target = os.getenv(
            "TARGET_ENV",
            "dev",
        ).lower()

        self.check_env()

        logger.debug(f"Environment set to {self.target}")

    def check_env(self: "EnvManager") -> None:
        """Check for required environment variables"""

        if os.getenv("TARGET_ENV", "test").lower() not in ["dev", "test", "prod"]:
            msg = """
            TARGET_ENV must be set to either `dev`, `test`, `prod`.
            """
            raise RuntimeError(msg)

    def check_key_in_env(self: "EnvManager", key_name: str | list[str]) -> None:
        if isinstance(key_name, str):
            key_name = [key_name]

        for i in key_name:
            if i not in os.environ:
                msg = f"{i} must be set as an environment variable"
                raise RuntimeError(msg)

    @property
    def return_target(self: "EnvManager") -> str:
        return self.target

    @property
    def return_schema(self: "EnvManager") -> str:
        """
        Return the schema to use for database operations.
        """
        schema_selections = {
            "prod": "PUBLISH",
            "test": "TRANSFORM",
            "dev": "SANDPIT",
        }

        return schema_selections.get(self.target, "SANDPIT")

    @property
    def sm_input_table(self: "EnvManager") -> str:
        return "final__customer_table"

    @property
    def sm_preferences_table(self: "EnvManager") -> str:
        return "final__customer_preferences_table"

from pure.logging import NimbusLogger

logger = NimbusLogger.get_logger(__name__)


async def log_message(message: str, user: str, timestamp: str) -> None:
    """
    Log a session message.

    The React template persists this to a Redshift ``<app>_logs`` table via
    ``database.composites.insert_data``. For this pilot we keep the same hook
    point in the request lifecycle but simply emit a structured log line, so the
    app runs standalone without a warehouse connection. Swap the body back to
    ``insert_data`` when wiring up a real database.
    """
    logger.info(f"[session] {timestamp} {user}: {message}")

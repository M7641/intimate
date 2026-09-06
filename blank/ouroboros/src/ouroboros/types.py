from pathlib import Path
from typing import TypedDict

class Artifact(TypedDict):
    ignore_files: list[str]
    path: Path | str

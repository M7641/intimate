from pathlib import Path

import yaml
from pydantic import ValidationError

from .pydantic_models import ConfigFile


def load_config(load_dir: Path) -> ConfigFile:
    if load_dir is None:
        load_dir = Path().cwd() / "proteus_config.yml"

    with Path.open(load_dir) as stream:
        configuration = yaml.safe_load(stream)

    try:
        configuration = ConfigFile(**configuration)
    except ValidationError:
        raise

    return configuration

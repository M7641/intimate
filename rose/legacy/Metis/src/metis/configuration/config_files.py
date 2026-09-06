import yaml
from metis.configuration import ConfigFile, Glossary
from metis.utility_tools.github_tools import (load_config_from_github,
                                              load_glossary_from_github)
from pydantic import ValidationError


def return_configuration_file(path: str, file_name: str) -> dict:
    """
    Open and load the specified configuration file.

    Args:
        path (str): The path to the directory containing the configuration file.
        config_name (str): The name of the configuration file.

    Returns:
        dict: The loaded configuration file as a dictionary.
    """

    try:
        with open(f"{path}/{file_name}") as stream:
            config_file = yaml.safe_load(stream)
    except FileNotFoundError:
        config_file = load_config_from_github()

    try:
        config_file = ConfigFile(**config_file)
    except ValidationError:
        raise

    for model_name, model_config in config_file.models_to_explain.items():
        model_config.name = model_name

    return config_file


def return_glossary(path: str, file_name: str) -> dict:
    """
    Open and load the specified glossary file.

    Args:
        path (str): The path to the directory containing the glossary file.
        glossary_name (str): The name of the glossary file.

    Returns:
        dict: The loaded glossary file as a dictionary.
    """

    try:
        with open(f"{path}/{file_name}") as stream:
            glossary = yaml.safe_load(stream)
    except FileNotFoundError:
        glossary = load_glossary_from_github()

    with open(f"{path}/{file_name}") as stream:
        glossary = yaml.safe_load(stream)

    try:
        glossary = Glossary(**glossary)
    except ValidationError:
        raise

    return glossary

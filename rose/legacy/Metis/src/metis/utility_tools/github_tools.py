import logging
import os

import yaml
from github import Auth, Github
from nimbus.exceptions import MissingEnvironmentVariableException
from nimbus.press import blocks

try:
    block_client = blocks.get_client()
except MissingEnvironmentVariableException:
    block_client = None
    logging.warning("Block client not initialized. Please set API_KEY env variable.")


def load_config_from_github() -> dict:
    """
    Load config from github.

    Returns
    -------
    dict
        Config.
    """
    parameters = {
        "config_repository": os.getenv("CONFIG_REPOSITORY"),
        "config_file_name": os.getenv("CONFIG_FILE_NAME"),
    }

    auth = Auth.Token(os.getenv("GITHUB_TOKEN"))
    github_login = Github(auth=auth)

    repo = github_login.get_repo(parameters["config_repository"])
    file = repo.get_contents(parameters["config_file_name"]).decoded_content

    with open("config.yaml", "wb") as f:
        f.write(file)

    with open("config.yaml") as stream:
        model_config = yaml.safe_load(stream)

    return model_config


def load_glossary_from_github() -> dict:
    """
    Load glossary from github.

    Returns
    -------
    dict
        Config.
    """
    parameters = {
        "config_repository": os.getenv("CONFIG_REPOSITORY"),
        "glossary_file": os.getenv("GLOSSARY_FILE"),
    }

    auth = Auth.Token(os.getenv("GITHUB_TOKEN"))
    github_login = Github(auth=auth)

    repo = github_login.get_repo(parameters["config_repository"])
    file = repo.get_contents(parameters["glossary_file"]).decoded_content

    with open("glossary.yaml", "wb") as f:
        f.write(file)

    with open("glossary.yaml") as stream:
        glossary = yaml.safe_load(stream)

    return glossary

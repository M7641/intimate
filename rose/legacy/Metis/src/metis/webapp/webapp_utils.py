import os


def get_prefix():
    """
    Return the prefix for the URL based on the environment and user.

    Returns:
        str: The prefix for the URL.
    """
    return (
        None
        if os.getenv("LIVE") != "DEVELOPMENT"
        else f"/user/{os.getenv('USER')}/proxy/8050/"
    )


def clean_strings_for_ui(list_of_strings: list[str]) -> list[str]:
    return [i.replace("_", " ").capitalize() for i in list_of_strings]

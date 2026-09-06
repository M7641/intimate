import math
import re

import numpy as np
import pandas as pd


def clean_feature_names(feature_names: list[str]):
    return [x.replace("_", " ").capitalize() for x in feature_names]


def try_float(value):
    try:
        return float(value)
    except ValueError:
        return None
    except TypeError:
        return None


def format_value(s, format_str):
    "Strips trailing zeros and uses a unicode minus sign."

    if not issubclass(type(s), str):
        s = format_str % s
    s = re.sub(r"\.?0+$", "", s)
    if s[0] == "-":
        s = "\u2212" + s[1:]
    return s


def try_fill_na(element):
    try:
        return element.fillna(element.mean())
    except TypeError:
        return None


def create_nice_category_name(category_name: str):
    """
    This function takes a category name and returns a nicer version of it.

    ARGS:
      category_name: The name of the category. Looks like: (x.left, x.right]
    """

    if math.ceil(category_name.left + 0.01) == math.floor(category_name.right):
        clean_category_name = f"{math.floor(category_name.right)}"
    else:
        clean_category_name = f"{math.ceil(category_name.left + 0.01)} to {math.floor(category_name.right)}"

    return clean_category_name


def build_plotting_data_with_quartiles(
    plotting_view: pd.DataFrame,
):
    plotting_view["actual_value_bins"] = pd.qcut(
        plotting_view["actual_value"].astype(float),
        q=6,
        precision=1,
    )
    plotting_view["actual_value_bins_to_sort"] = plotting_view[
        "actual_value_bins"
    ].cat.rename_categories(
        lambda x: int(x.left),
    )
    plotting_view["actual_value_bins"] = plotting_view[
        "actual_value_bins"
    ].cat.rename_categories(lambda x: f"{create_nice_category_name(x)}")

    return plotting_view


def build_plotting_data_without_quartiles(
    plotting_view: pd.DataFrame,
):
    plotting_view["actual_value_bins"] = pd.cut(
        plotting_view["actual_value"].astype(float),
        bins=6,
        precision=1,
    )
    plotting_view["actual_value_bins_to_sort"] = plotting_view[
        "actual_value_bins"
    ].cat.rename_categories(
        lambda x: (int(x.left)),
    )
    plotting_view["actual_value_bins"] = plotting_view[
        "actual_value_bins"
    ].cat.rename_categories(lambda x: f"{create_nice_category_name(x)}")

    return plotting_view


def convert_nan_like_values_to_nan(value):
    nan_like_values = ["nan", "NaN", "none", "None"]
    if value in nan_like_values:
        return np.nan
    return value

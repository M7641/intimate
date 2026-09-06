import pandas as pd


def return_class_weights(
    target_data: pd.DataFrame, target_column: str = "target"
) -> dict:
    weights_pre = (
        target_data.groupby(target_column)
        .size()
        .rename("positives")
        .reset_index()
        .assign(
            total=len(target_data),
            classes=target_data[target_column].nunique(),
        )
    )

    weights = (
        1 / weights_pre["positives"] * weights_pre["total"] / weights_pre["classes"]
    )
    class_weight = dict(zip(weights_pre[target_column], weights))

    return class_weight


def return_response_variable_type(target: pd.DataFrame) -> str:
    """
    Returns the type of the response variable based on its data type and unique values.

    Args:
        target (pd.DataFrame): The target variable as a pandas DataFrame.

    Returns:
        str: The type of the response variable. Possible values are "binary_classifier", "multi_classifier", or "regression".

    Raises:
        Exception: If the response variable is of an invalid data type.
    """
    target_data_type = pd.api.types.infer_dtype(target["target"], skipna=True)

    if target_data_type == "integer":
        unique_target_values = len(target["target"].unique())
        target_type = (
            "binary_classifier" if unique_target_values == 2 else "multi_classifier"
        )
    elif target_data_type == "floating":
        target_type = "regression"
    else:
        msg = "Invalid Response Variable."
        raise Exception(msg)

    return target_type


def return_number_of_classes(target: pd.DataFrame) -> int:
    """
    Returns the number of classes in the target variable.

    Args:
        target (pd.DataFrame): The target variable as a pandas DataFrame.

    Returns:
        int: The number of classes in the target variable.
    """
    return len(target["target"].unique())

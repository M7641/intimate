from pathlib import Path

import pandas as pd


def save_to_parquet(data: pd.DataFrame, dir_name: str | Path, data_name: str) -> None:
    """Save the given DataFrame to a parquet file.

    Args:
    ----
        data (pd.DataFrame): The DataFrame to be saved.
        dir_name (str): The directory where the parquet file will be saved.
        data_name (str): The name of the parquet file.

    Returns:
    -------
        None
    """
    save_path = Path(dir_name).resolve() / "data"
    save_path.mkdir(parents=True, exist_ok=True)
    file_path = save_path / f"{data_name}.parquet.gzip"
    data.columns = data.columns.astype(str)
    data.to_parquet(file_path, compression="gzip")

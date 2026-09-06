import io
import os
import pickle
import shutil
import tarfile
import tempfile
from pathlib import Path
from typing import Any

import polars as pl
import boto3
import warnings


class S3Manager:
    def __init__(self: "S3Manager") -> None:
        self.bucket_name = os.getenv("DATA_LAKE")
        self.tenant = os.getenv("TENANT")
        self.s3 = boto3.client("s3")

    def load_pickle(self: "S3Manager", key: str) -> Any:
        data = self.s3.get_object(Bucket=self.bucket_name, Key=key)["Body"].read()
        return pickle.loads(data)  # noqa: S301

    def save_pickle(self: "S3Manager", key: str, data: Any) -> None:
        with tempfile.TemporaryFile() as fp:
            pickle.dump(data, fp)
            fp.seek(0)
            data = fp.read()

        self.s3.put_object(Bucket=self.bucket_name, Key=key, Body=data)

    def save_file(self: "S3Manager", key: str, file_name: Path) -> None:
        with file_name.open("rb") as f:
            data = f.read()

        self.s3.put_object(Bucket=self.bucket_name, Key=key, Body=data)

    def load_file(self: "S3Manager", key: str, file_name: Path) -> None:
        warnings.warn(
            "load_file is deprecated, use download_file instead",
            DeprecationWarning,
            stacklevel=2
        )
        data = self.s3.get_object(Bucket=self.bucket_name, Key=key)["Body"].read()
        file_name.parent.mkdir(parents=True, exist_ok=True)
        with file_name.open("wb") as f:
            f.write(data)

    def download_file(self: "S3Manager", key: str, file_name: Path) -> None:
        """
        Download a file from S3 and save it to the specified file path.
        This method will create the necessary directories if they do not exist.
        """
        data = self.s3.get_object(Bucket=self.bucket_name, Key=key)["Body"].read()
        file_name.parent.mkdir(parents=True, exist_ok=True)
        with file_name.open("wb") as f:
            f.write(data)

    def load_to_pl(
        self: "S3Manager",
        key: str,
        data_schema: dict[str, pl.datatypes.DataType] | None = None,
        schema_overrides: dict[str, pl.datatypes.DataType] | None = None,
    ) -> Any:
        """
        Load a file from S3 and return its content as a polars DataFrame.
        This method assumes the file is in CSV format.
        """
        data = self.s3.get_object(Bucket=self.bucket_name, Key=key)["Body"].read()
        return pl.read_csv(
            io.BytesIO(data),
            schema=data_schema,
            schema_overrides=schema_overrides,
            infer_schema_length=None
        )

    def save_directory(self: "S3Manager", key: str, directory: Path) -> None:
        key = f"{key}.tar.gz"

        with (
            tempfile.NamedTemporaryFile("wb", suffix=".tar.gz", delete=False) as fp,
            tarfile.open(fileobj=fp, mode="w:gz") as tar,
        ):
            tar.add(directory, arcname=directory.name)

        with Path(fp.name).open("rb") as f:
            self.s3.upload_fileobj(Fileobj=f, Bucket=self.bucket_name, Key=key)

        shutil.rmtree(directory)

    def load_directory(self: "S3Manager", key: str, directory: Path) -> None:
        key = f"{key}.tar.gz"
        data = self.s3.get_object(Bucket=self.bucket_name, Key=key)["Body"].read()
        with tempfile.TemporaryFile() as fp:
            fp.write(data)
            fp.seek(0)
            # https://docs.astral.sh/ruff/rules/tarfile-unsafe-members/ for fix in 3.12
            with tarfile.open(fileobj=fp, mode="r") as tar:
                tar.extractall(directory)  # noqa: S202

    def list_s3_objects(self: "S3Manager", prefix: str) -> dict:
        """Lists all S3 objects that have been saved in a given environment for a
        given day and Day 0 date."""
        return self.s3.list_objects_v2(
            Bucket=self.bucket_name,
            Prefix=prefix,
        )

    def upload_string_to_s3(self: "S3Manager", data: str, key: str) -> None:
        self.s3.put_object(Bucket=self.bucket_name, Key=key, Body=data)

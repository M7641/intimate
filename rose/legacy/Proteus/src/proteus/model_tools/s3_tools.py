import contextlib
import datetime
import logging
import os
import re
import shutil
import tarfile
import tempfile
from pathlib import Path

import boto3
import joblib
import pytz


def upload_to_s3(
    file: str,
    model_name: str,
    object_name: str,
) -> None:
    tenant = os.environ.get("TENANT")
    s3_bucket = os.environ.get("DATA_LAKE")
    s3 = boto3.client("s3")

    timestamp = f"{datetime.datetime.now(tz=pytz.utc)}"
    timestamp = re.sub("[^A-Za-z0-9]+", "", timestamp)

    s3_key = (
        f"{tenant}/datascience/models/"
        f"{model_name}/"
        f"{object_name}/"
        f"{model_name}_{object_name}_{timestamp}.tar.gz"
    )

    directory_saved = Path("./temp").resolve()
    directory_saved.mkdir(parents=True, exist_ok=True)
    joblib.dump(file, f"{directory_saved!s}/{object_name}.joblib", compress=9)

    with tempfile.NamedTemporaryFile("wb", suffix=".tar.gz", delete=False) as f:
        with tarfile.open(fileobj=f, mode="w:gz") as tar:
            tar.add(
                str(directory_saved),
                arcname=os.path.basename(str(directory_saved)),
            )

    with open(f.name, "rb") as tar:
        try:
            s3.upload_fileobj(tar, s3_bucket, s3_key)
        except Exception:
            raise

    logging.info("1: Running Clean up!")
    if directory_saved.is_dir():
        shutil.rmtree(str(directory_saved))
    logging.info("2: Clean up ran!")


def upload_directory_to_s3(
    save_dir: Path | str,
    model_name: str,
    object_name: str,
) -> None:
    save_dir = str(save_dir)
    tenant = os.environ.get("TENANT")
    s3_bucket = os.environ.get("DATA_LAKE")

    s3 = boto3.client("s3")
    timestamp = f"{datetime.datetime.now(tz=pytz.utc)}"
    timestamp = re.sub("[^A-Za-z0-9]+", "", timestamp)

    s3_key = (
        f"{tenant}/datascience/models/"
        f"{model_name}/"
        f"{object_name}/"
        f"{model_name}_{object_name}_{timestamp}.tar.gz"
    )

    with tempfile.NamedTemporaryFile("wb", suffix=".tar.gz", delete=False) as f:
        with tarfile.open(fileobj=f, mode="w:gz") as tar:
            tar.add(save_dir, arcname=os.path.basename(save_dir))

    with open(f.name, "rb") as tar:
        try:
            s3.upload_fileobj(tar, s3_bucket, s3_key)
        except Exception:
            raise


def get_last_modified(obj):
    return int(obj["LastModified"].strftime("%s"))


def download_most_recent_tar_file(
    save_dir: str | Path,
    model_name: str,
    object_name: str,
) -> None:
    s3 = boto3.client("s3")
    s3_bucket = os.environ.get("DATA_LAKE")
    save_dir = str(save_dir)

    tenant = os.environ.get("TENANT")
    s3_key_for_folder = (
        f"{tenant}/datascience/models/" f"{model_name}/" f"{object_name}/"
    )

    objects_in_folder = s3.list_objects_v2(Bucket=s3_bucket, Prefix=s3_key_for_folder)[
        "Contents"
    ]

    last_added = next(
        obj["Key"]
        for obj in sorted(objects_in_folder, key=get_last_modified, reverse=True)
    )

    try:
        Path(save_dir).mkdir(parents=True, exist_ok=True)
        s3.download_file(
            s3_bucket,
            last_added,
            save_dir + "temporary_file",
        )

        with tarfile.open(save_dir + "temporary_file") as tar:
            tar.extractall(save_dir)

        with contextlib.suppress(OSError):
            os.remove(save_dir + "temporary_file")

    except Exception:
        raise

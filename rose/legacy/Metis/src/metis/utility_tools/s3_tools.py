import contextlib
import datetime
import logging
import os
import shutil
import tarfile
import tempfile
from pathlib import Path

import boto3
import joblib
import pytz


def upload_to_s3(file: str, model_name: str, object_name: str) -> None:
    """
    Uploads a file to S3 with the specified model name and object name.

    Args:
        file (str): The path to the file to be uploaded.
        model_name (str): The name of the model.
        object_name (str): The name of the object.

    Returns:
        None
    """
    s3_bucket = os.environ.get("DATA_LAKE")

    directory_saved = Path("./temp").resolve()
    directory_saved.mkdir(parents=True, exist_ok=True)
    joblib.dump(file, f"{directory_saved}/{object_name}.joblib")

    s3 = boto3.client("s3")
    timestamp = datetime.datetime.now(tz=pytz.utc).strftime("%Y%m%d%H%M%S")

    s3_key = (
        f"{os.environ.get('TENANT')}/datascience/models/"
        f"{model_name}/{object_name}/"
        f"{model_name}_{object_name}_{timestamp}.tar.gz"
    )

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

    logging.info("Running Clean up!")
    shutil.rmtree(str(directory_saved))


def get_last_modified(obj):
    return int(obj["LastModified"].strftime("%s"))


def download_most_recent_tar_file(
    save_dir: str,
    s3_key: str = None,
    s3_bucket: str = os.environ.get("DATA_LAKE"),
) -> None:
    """
    Downloads the most recent tar file for a given model and object name from an S3 bucket.

    Parameters:
        model_name (str): The name of the model.
        object_name (str): The name of the object.
        save_dir (str): The directory where the downloaded file will be saved.

    Returns:
        None
    """
    s3 = boto3.client("s3")
    objects_in_folder = s3.list_objects_v2(Bucket=s3_bucket, Prefix=s3_key)["Contents"]

    last_added = max(objects_in_folder, key=lambda obj: obj["LastModified"])["Key"]

    try:
        Path(save_dir).mkdir(parents=True, exist_ok=True)
        s3.download_file(s3_bucket, last_added, f"{save_dir}/temporary_file")

        with tarfile.open(f"{save_dir}/temporary_file") as tar:
            tar.extractall(save_dir)

        with contextlib.suppress(OSError):
            os.remove(f"{save_dir}/temporary_file")

    except Exception:
        raise

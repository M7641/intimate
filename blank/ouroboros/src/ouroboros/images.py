import json
import os
import tempfile
import zipfile
from pathlib import Path
from typing import Iterator

import requests
from pathspec import PathSpec
from pure.logging import NimbusLogger

from ouroboros.types import Artifact

logger = NimbusLogger(name=__name__).logger


def get_files_to_include(path: Path, ignore_files: list[str]) -> Iterator[str]:
    raw_patterns: list[str] = []
    for ignore_file in ignore_files:
        ignore_file_path = path / ignore_file
        if not ignore_file_path.exists():
            continue
        for line in ignore_file_path.read_text().splitlines():
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            raw_patterns.append(stripped)

    if not raw_patterns:
        ignore_spec = None
    else:
        ignore_spec = PathSpec.from_lines("gitwildmatch", raw_patterns)

    for root, dirs, files in os.walk(str(path), followlinks=True):
        rel_root = os.path.relpath(root, str(path))
        if rel_root == ".":
            rel_root = ""

        if ignore_spec:
            dirs[:] = [
                d
                for d in dirs
                if not ignore_spec.match_file(
                    (f"{rel_root}/{d}" if rel_root else d) + "/"
                )
            ]

        for f in files:
            file_path = f"{rel_root}/{f}" if rel_root else f
            if ignore_spec is None or not ignore_spec.match_file(file_path):
                yield file_path


class Images:
    base_url = "https://service.nimbus.example/image-management/api/v2"

    def parse_body_for_multipart_request(self: "Images", body: dict) -> dict[str, str]:
        return {
            key: (value if isinstance(value, str) else json.dumps(value))
            for (key, value) in body.items()
        }

    def list_images(self: "Images"):
        # Nimbus paginates GET /images. Reading only the first page would make any
        # image beyond the 75th "disappear" from the list, so
        # create_or_update_image_version would recreate an existing image
        # (-> 400 "name is already in use"). Loop until pageCount instead.
        images: list = []
        page = 1
        while True:
            response = requests.get(
                f"{self.base_url}/images",
                params={"pageSize": 75, "pageNumber": page},
                headers={"Authorization": os.getenv("API_KEY")},
                timeout=30,
            )

            if response.status_code != 200:
                raise RuntimeError(f"Failed to list images: {response.text}")

            body = response.json()
            batch = body["images"]
            images.extend(batch)

            # pageCount is the authoritative bound; without it, stop after the
            # current page. An empty page also breaks the loop, as a guard.
            page_count = body.get("pageCount", page)
            if page >= page_count or not batch:
                break
            page += 1

        return images

    def list_image_versions(self: "Images", image_id: str) -> list:
        params = {
            "pageSize": None,
            "searchTerm": None,
            "status": None,
            "lastBuildStatus": None,
            "tags": None,
        }

        response = requests.get(
            f"{self.base_url}/images/{image_id}/versions",
            headers={"Authorization": os.getenv("API_KEY")},
            timeout=30,
            params=params,
        )

        if response.status_code != 200:
            raise RuntimeError(f"Failed to list image versions: {response.text}")

        return response.json().get("versions", [])

    def describe_image(self: "Images", image_id: int) -> dict:
        response = requests.get(
            f"{self.base_url}/images/{image_id}",
            headers={"Authorization": os.getenv("API_KEY")},
            timeout=30,
        )

        if response.status_code != 200:
            raise RuntimeError(f"Failed to describe image: {response.text}")

        return response.json()

    def describe_version(self: "Images", image_id: str, version_id: str) -> dict:
        response = requests.get(
            f"{self.base_url}/images/{image_id}/versions/{version_id}",
            headers={"Authorization": os.getenv("API_KEY")},
            timeout=30,
        )

        if response.status_code != 200:
            raise RuntimeError(f"Failed to describe image version: {response.text}")

        return response.json()

    def create_image(
        self: "Images",
        body: dict,
        artifact: Artifact = {"ignore_files": [".dockerignore"], "path": "."},
    ) -> dict:

        if "type" not in body:
            raise ValueError("Image type is required in body")

        if body["type"] not in [
            "api",
            "webapp",
            "workflow",
            "workspace-python",
            "workspace-r",
        ]:
            raise ValueError(
                f"Invalid image type: {body['type']}. Must be one of:"
                "api, webapp, workflow, workspace-python, workspace-r"
            )

        artifact_path = Path(artifact.get("path", "."))

        ignore_files = artifact.get("ignore_files", [".dockerignore"])
        body = self.parse_body_for_multipart_request(body)
        included_files = get_files_to_include(artifact_path, ignore_files)

        with tempfile.NamedTemporaryFile(delete=False, suffix=".zip") as temp_zip:
            zip_path = temp_zip.name

        # Create zip file from the artifact path
        with zipfile.ZipFile(
            zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9
        ) as zipf:
            if not artifact_path.exists():
                raise FileNotFoundError(
                    f"Artifact path does not exist: {artifact_path}"
                )

            if artifact_path.is_dir():
                for file_path in included_files:
                    full_path = artifact_path / file_path
                    if full_path.is_file():
                        zipf.write(full_path, file_path)

            if (
                artifact_path.is_file()
                and "dockerfile" not in artifact_path.name.lower()
            ):
                raise ValueError(
                    f"If artifact path is a file, it must be a Dockerfile. Got: {artifact_path.name}"
                )

            if artifact_path.is_file():
                zipf.write(artifact_path, artifact_path.name)

        with open(zip_path, "rb") as zip_file:
            files = {"artifact": ("artifact.zip", zip_file, "application/zip")}

            file_size = os.path.getsize(zip_path)
            logger.info(
                f"Uploading artifact: {file_size:,} bytes ({file_size / 1024 / 1024:.2f} MB)"
            )

            response = requests.post(
                f"{self.base_url}/images",
                headers={"Authorization": os.getenv("API_KEY")},
                timeout=30,
                data=body,
                files=files,
            )

        os.unlink(zip_path)

        if response.status_code != 201:
            raise RuntimeError(f"Failed to create image: {response.text}")

        return response.json()

    def create_version(
        self: "Images",
        image_id: str,
        body: dict,
        artifact: Artifact = {"ignore_files": [".dockerignore"], "path": "."},
    ) -> dict:
        path = Path(artifact.get("path", "."))

        ignore_files = artifact.get("ignore_files", [".dockerignore"])
        body = self.parse_body_for_multipart_request(body)

        included_files = get_files_to_include(path, ignore_files)

        with tempfile.NamedTemporaryFile(delete=False, suffix=".zip") as temp_zip:
            zip_path = temp_zip.name

        # Create zip file from the artifact path
        with zipfile.ZipFile(
            zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9
        ) as zipf:
            artifact_path = Path(path)
            if artifact_path.is_file():
                zipf.write(artifact_path, artifact_path.name)
            elif artifact_path.is_dir():
                for file_path in included_files:
                    full_path = artifact_path / file_path
                    if full_path.is_file():
                        zipf.write(full_path, file_path)

        with open(zip_path, "rb") as zip_file:
            files = {"artifact": ("artifact.zip", zip_file, "application/zip")}

            file_size = os.path.getsize(zip_path)
            logger.info(
                f"Uploading artifact: {file_size:,} bytes ({file_size / 1024 / 1024:.2f} MB)"
            )

            response = requests.post(
                f"{self.base_url}/images/{image_id}/versions",
                headers={"Authorization": os.getenv("API_KEY")},
                timeout=30,
                data=body,
                files=files,
            )

        os.unlink(zip_path)

        if response.status_code != 201:
            raise RuntimeError(f"Failed to create image version: {response.text}")

        return response.json()

    def create_or_update_image_version(
        self: "Images",
        body: dict,
        artifact: Artifact,
    ) -> dict:
        image_name = body.get("name")
        if not image_name:
            raise ValueError("Image name is required in body")

        if isinstance(artifact["ignore_files"], str):
            artifact["ignore_files"] = [artifact["ignore_files"]]

        images = self.list_images()
        existing_image = next(
            (img for img in images if img["name"] == image_name), None
        )

        path = Path(artifact.get("path", ".")).absolute()
        logger.info(f"Creating image version with artifact path: {path}")

        if existing_image:
            return self.create_version(existing_image["id"], body, artifact)
        else:
            return self.create_image(body, artifact)

    def delete_version(self: "Images", image_id: str, version_id: str) -> None:
        response = requests.delete(
            f"{self.base_url}/images/{image_id}/versions/{version_id}",
            headers={"Authorization": os.getenv("API_KEY")},
            timeout=30,
        )

        return response.json()

    def delete_all_but_x_images(
        self: "Images", image_id: str, keep_last_n: int = 3
    ) -> None:
        image_versions = list(self.list_image_versions(image_id))

        if len(image_versions) > keep_last_n:
            remove_most_recent = sorted([i["createdAt"] for i in image_versions])
            versions_to_pruge = [
                i
                for i in image_versions
                if i["createdAt"] in remove_most_recent[:-keep_last_n]
            ]
            for i in versions_to_pruge:
                self.delete_version(image_id=image_id, version_id=i["id"])


def find_parent(path: Path, target_parent: str) -> Path:
    for parent in [path, *list(path.parents)]:
        if parent.name == target_parent:
            return parent

    raise FileNotFoundError(
        f"Parent directory '{target_parent}' not found for path '{path}'."
    )


def deploy_image(body: dict, artifact: Artifact | None = None) -> tuple[int, int]:
    if artifact is None:
        artifact = {"ignore_files": [".dockerignore"], "path": "."}

    image_client = Images()

    tenant = os.getenv("TENANT")

    logger.info(f"Deploying image for tenant: {tenant}")

    response = image_client.create_or_update_image_version(
        body=body,
        artifact=artifact,
    )

    image_client.delete_all_but_x_images(
        image_id=response["imageId"],
        keep_last_n=3,
    )

    return response["imageId"], response["versionId"]

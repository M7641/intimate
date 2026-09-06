import os
import re
import stat
from datetime import datetime
from pathlib import Path

import paramiko
from pure.logging import NimbusLogger

logger = NimbusLogger.get_logger(__name__)


class SFTPManager:
    """
    Commandered from the Deng team's SFTP implementation.
    https://github.com/nimbus-labs/deng/blob/b9bfff5581374e0ef2104e429d298f0de27c618e/src/deng/SFTP/SFTP.py
    Thank you Charlie!

    The intention is to use this class as a context manager:
    with SFTPManager() as sftp_manager:
        sftp_manager.upload_file_from_local(...)

    This should ensure that connections are properly closed after use.

    This assumes keys are in RSA format:
    paramiko.Ed25519Key.from_private_key_file(...)  # for Ed25519
    paramiko.ECDSAKey.from_private_key_file(...)    # for ECDSA

    Authentication:
        Supports either password or private key authentication.
        Credentials can be passed directly or read from environment variables.

        Parameters take precedence over environment variables:
        - hostname / SFTP_HOSTNAME
        - username / SFTP_USERNAME
        - password / SFTP_PASSWORD
        - private_key_path / SFTP_PRIVATE_KEY_PATH
    """

    def __init__(
        self,
        hostname: str | None = None,
        username: str | None = None,
        password: str | None = None,
        private_key_path: str | None = None,
        port: int = 22,
        disable_rsa_sha2: bool = True,
        compress: bool = False,
        window_size: int | None = None,
    ) -> None:
        self.port = port
        self.hostname = hostname or os.getenv("SFTP_HOSTNAME")
        self.username = username or os.getenv("SFTP_USERNAME")
        self.password = password or os.getenv("SFTP_PASSWORD")
        self.private_key_path = private_key_path or os.getenv("SFTP_PRIVATE_KEY_PATH")
        self.disable_rsa_sha2 = disable_rsa_sha2
        self.compress = compress
        self.window_size = window_size
        self._ssh_client = paramiko.SSHClient()
        self._ssh_client.set_missing_host_key_policy(paramiko.AutoAddPolicy())

        if self.hostname is None or self.username is None:
            raise ValueError("hostname and username must be provided.")

        if self.password is None and self.private_key_path is None:
            raise ValueError("Either password or private_key_path must be provided.")

        self._connect()
        self.client = self._ssh_client.open_sftp()
        self._configure_channel()

    def _connect(self) -> None:
        """Establishes SSH connection using either password or private key authentication."""
        connect_kwargs = {
            "hostname": self.hostname,
            "username": self.username,
            "port": self.port,
        }

        if self.disable_rsa_sha2:
            connect_kwargs["disabled_algorithms"] = {
                "pubkeys": ["rsa-sha2-256", "rsa-sha2-512"]
            }

        if self.compress:
            connect_kwargs["compress"] = True

        if self.private_key_path:
            pkey = paramiko.RSAKey.from_private_key_file(
                self.private_key_path,
            )
            connect_kwargs["pkey"] = pkey
        else:
            connect_kwargs["password"] = self.password

        self._ssh_client.connect(**connect_kwargs)

    def _configure_channel(self) -> None:
        """Configures the SFTP channel for improved performance."""
        if self.window_size is not None:
            channel = self.client.get_channel()
            if channel is not None:
                channel.in_window_size = self.window_size
                channel.out_window_size = self.window_size
                channel.in_max_packet_size = min(self.window_size, 32768)
                channel.out_max_packet_size = min(self.window_size, 32768)

    def login(self) -> None:
        """Logs into the Server using the client."""
        if self._ssh_client is None:
            self._ssh_client = paramiko.SSHClient()
            self._ssh_client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
            self._connect()

        if not self.client:
            self.client = self._ssh_client.open_sftp()
            self._configure_channel()

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        if self.client:
            self.client.close()

        if self._ssh_client:
            self._ssh_client.close()

    def __enter__(self) -> "SFTPManager":
        return self

    def upload_file_from_local(
        self,
        file_name: Path | str,
        directory_in_sftp: str,
        remove_local_file: bool = True,
        overwrite_if_exists: bool = True,
    ) -> None:
        """
        Uploads a single files from a specific locations to a target folder on the server.

        check if directory exists. if not raise error.
        check if file exists. if so, depending on overwrite_if_exists either skip or overwrite.
        """

        if isinstance(file_name, str):
            file_name = Path(file_name)

        remote_path = f"{directory_in_sftp}/{file_name.name}"

        if file_name.exists() is False:
            raise FileNotFoundError(
                f"Local file {file_name} does not exist; cannot upload."
            )

        self.check_remote_directory_exists(remote_path)

        if not overwrite_if_exists and self.check_remote_file_exists(
            remote_path, allow_not_exist=overwrite_if_exists
        ):
            logger.error(
                "Overwrite set to False, and remote file already exists; skipping"
            )

        self.client.put(file_name.resolve(), remote_path, callback=None, confirm=True)

        if remove_local_file and file_name.exists():
            file_name.unlink()

    def check_remote_file_exists(
        self, remote_path: str, allow_not_exist: bool = False
    ) -> bool:
        """
        Attempts to get the attributes of a remote file to check it exists.

        Notes
        ------
        Used to check if a remote file exists;
            - Can be used to prevent overwrites if target file already exists when Uploading
            - Can fail here if trying to Download a non-existent file

        """
        try:
            self.client.stat(remote_path)
            logger.debug(f"File exists: {remote_path}")
            return True

        except FileNotFoundError:
            logger.debug(f"Could not get stats for file: {remote_path}")
            if allow_not_exist:
                logger.debug("allow_not_exist is True, continuing..")
                return False

            else:
                logger.debug("allow_not_exist is False, raising error..")
                raise FileNotFoundError(f"File does not exist on server: {remote_path}")

    def check_remote_directory_exists(self, remote_path: str) -> None:
        """
        Inspects the server to check if a directory exists.

        Notes
        ------
        In case a dir needs to be created before writing.
        """
        remote_path_dir = os.path.split(remote_path)[0]
        remote_path_lstat = self.client.lstat(remote_path_dir)

        if remote_path_lstat.st_mode is None:
            raise ValueError(f"Could not determine file type for: {remote_path_dir}")

        is_dir = stat.S_ISDIR(remote_path_lstat.st_mode)
        if is_dir:
            logger.debug("Directory exists")

        else:
            raise ValueError(
                f"Given remote_path is a file not a directory!: {remote_path_dir}"
            )

    def return_list_of_files(
        self, sftp_directory: str = ".", pattern: str | None = None
    ) -> list[str]:
        """
        Lists the contents of the directory.

        Notes
        ------
        If a path is not specified will list current working directory.
        """
        response = self.client.listdir(sftp_directory)

        if pattern:
            response = self._filter_regex_string(response, pattern)

        return response

    @staticmethod
    def _filter_regex_string(file_list: list[str], file_regex_string: str) -> list[str]:
        """Simple method that filters a list of strings for a partial match with a string."""
        reg = re.compile(file_regex_string)
        filtered_file_list = []
        for file in file_list:
            if reg.search(file):
                logger.info(f"Matched file: {file} - {reg}")
                filtered_file_list.append(file)
        return filtered_file_list

    def create_directory(self, remote_path: str) -> None:
        """Creates a directory in the server"""
        logger.info(f"Creating directory: {remote_path}")
        self.client.mkdir(remote_path)

    def remove_file_from_sftp(self, remote_path: str) -> None:
        """
        Removes a file from the server.

        Notes
        ------
        - Checks if the remote file exists first, if not then job done!
        """

        self.check_remote_file_exists(remote_path, allow_not_exist=True)
        logger.info(f"Removing file: {remote_path}")
        try:
            self.client.remove(remote_path)

        except Exception as e:
            logger.error(e)

    def download_file_from_sftp(
        self,
        sftp_directory: str,
        file_name: str,
        local_path: Path | None = None,
    ) -> None:
        if local_path is None:
            local_path = Path.cwd() / file_name

        try:
            self.client.get(
                f"{sftp_directory}/{file_name}",
                local_path.resolve(),
                callback=None,
                prefetch=True,
            )

        except Exception as e:
            logger.error(e)

    def get_last_modified_time(
        self,
        sftp_directory: str,
        file_name: str,
    ) -> int | None:
        """
        Gets the last modified time of a file on the SFTP server.

        Notes
        ------
        - Returns the last modified time as a timestamp float.
        """
        remote_path = f"{sftp_directory}/{file_name}"
        file_attr = self.client.stat(remote_path)
        return file_attr.st_mtime

    def list_files_by_date_range(
        self,
        sftp_directory: str,
        start_date: datetime,
        end_date: datetime,
        pattern: str | None = None,
        include_metadata: bool = False,
    ) -> list[str] | list[dict]:
        """
        Lists files in a directory that were uploaded/modified within a date range.

        Parameters
        ----------
        sftp_directory : str
            The remote directory to search in.
        start_date : datetime
            The start of the date range (inclusive).
        end_date : datetime
            The end of the date range (inclusive).
        pattern : str | None
            Optional regex pattern to filter file names.
        include_metadata : bool
            If True, returns a list of dicts with file name and modification time.
            If False, returns just a list of file names.

        Returns
        -------
        list[str] | list[dict]
            List of file names, or list of dicts with 'name' and 'modified_time' keys.

        Notes
        ------
        - Uses listdir_attr() for efficiency (single call instead of per-file stat).
        - Filters out directories, only returns regular files.
        - The modification time reflects when the file was last modified/uploaded.
        """
        start_timestamp = start_date.timestamp()
        end_timestamp = end_date.timestamp()

        file_attrs = self.client.listdir_attr(sftp_directory)

        matched_files = []
        for file_attr in file_attrs:
            if file_attr.st_mode is None:
                continue

            if not stat.S_ISREG(file_attr.st_mode):
                continue

            if file_attr.st_mtime is None:
                continue

            if start_timestamp <= file_attr.st_mtime <= end_timestamp:
                if pattern and not re.search(pattern, file_attr.filename):
                    continue

                if include_metadata:
                    matched_files.append(
                        {
                            "name": file_attr.filename,
                            "modified_time": datetime.fromtimestamp(file_attr.st_mtime),
                        }
                    )
                else:
                    matched_files.append(file_attr.filename)

                logger.debug(
                    f"File matched date range: {file_attr.filename} "
                    f"(modified: {datetime.fromtimestamp(file_attr.st_mtime)})"
                )

        logger.info(
            f"Found {len(matched_files)} files in {sftp_directory} "
            f"between {start_date} and {end_date}"
        )

        return matched_files

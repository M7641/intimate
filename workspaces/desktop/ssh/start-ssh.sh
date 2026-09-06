#!/usr/bin/env bash
#
# start-ssh [PORT] — run an SSH server for Zed's SSH remoting (option A).
#
# Your LOCAL Zed connects here and renders with your real GPU, while the code/compute
# live in this workspace — same idea as the VS Code / Cursor tunnels, native speed.
# Works for any SSH client too. Default port 2222.
#
# Setup:
#   1) add your public key to ~/.ssh/authorized_keys
#   2) run `start-ssh`
#   3) from your machine: zed ssh://<user>@<host>:2222/path/to/project
#      (Zed auto-installs its headless server component on first connect)
#
# Reachability depends on how Nimbus exposes/forwards the port — validate on Nimbus.
#
set -euo pipefail
PORT="${1:-2222}"

if [ "$(id -u)" -ne 0 ]; then SUDO=sudo; else SUDO=""; fi
$SUDO mkdir -p /run/sshd
$SUDO ssh-keygen -A                       # generate host keys if missing

echo "sshd on port ${PORT} (pubkey only). Add your key to ~/.ssh/authorized_keys."
exec $SUDO /usr/sbin/sshd -D -p "${PORT}" \
    -o PasswordAuthentication=no \
    -o PubkeyAuthentication=yes \
    -o PermitRootLogin=prohibit-password

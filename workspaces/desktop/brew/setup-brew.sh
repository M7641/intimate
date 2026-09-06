#!/usr/bin/env bash
#
# setup-brew — install Homebrew into your PERSISTENT home (~/.linuxbrew), no root needed.
#
# Why here and not system-wide: only /home/<you> persists on this platform, and Homebrew
# refuses to run unless it owns its prefix (and there's no sudo here). Installing into your
# home satisfies both — brew AND everything you `brew install` then survive restarts.
#
# Trade-off: a non-default prefix means some formulae build from source (gcc/build-essential
# is installed, so that works, just slower than bottles).
#
# Run once. New shells auto-load brew onto PATH (wired in /etc/bash.bashrc & zshrc).
#
set -euo pipefail

PREFIX="$HOME/.linuxbrew"
if [ -x "$PREFIX/bin/brew" ]; then
  echo "Homebrew already installed at $PREFIX"
else
  echo "Installing Homebrew into $PREFIX (rootless, persists in your home)…"
  mkdir -p "$PREFIX"
  curl -fsSL https://github.com/Homebrew/brew/tarball/master \
    | tar xz --strip-components 1 -C "$PREFIX"
fi

eval "$("$PREFIX/bin/brew" shellenv)"
brew update --force --quiet || true

echo
echo "Done. Open a new shell (or: eval \"\$(~/.linuxbrew/bin/brew shellenv)\"), then: brew install <pkg>"

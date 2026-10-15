#!/usr/bin/env bash
# Copy the Guards of Atlantis II images from Stats of Atlantis into assets/.
# A shallow, sparse clone: only the image folders kept here are downloaded.
set -euo pipefail
cd "$(dirname "$0")/.."

folders=(cards avatars avatars_full logos spell_cards stat_icons)

src=$(mktemp -d)
trap 'rm -rf "$src"' EXIT
git clone --quiet --depth 1 --filter=blob:none --sparse \
  https://github.com/QuasiStellar/Stats-of-Atlantis.git "$src"
git -C "$src" sparse-checkout set "${folders[@]/#/src/lib/images/}"
images="$src/src/lib/images"

mkdir -p assets
for folder in "${folders[@]}"; do
  rm -rf "assets/$folder"
  cp -r "$images/$folder" "assets/$folder"
done
# The loose files are the pieces the upstream site paints a card face from.
rm -rf assets/card_parts
mkdir assets/card_parts
cp "$images"/*.png assets/card_parts/

# Which upstream commit this copy is.
git -C "$src" log -1 --format='%H %cs' > assets/SOURCE
cat assets/SOURCE
du -sh assets

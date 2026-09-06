#!/bin/sh
#
# set-wallpaper — apply the default desktop wallpaper in XFCE.
#
# Run from XFCE autostart. The monitor/output name varies under VNC, so instead of hardcoding
# a backdrop path we set every existing last-image property, then reload xfdesktop.
#
WP="${WORKSPACE_WALLPAPER:-/usr/share/backgrounds/tokyonight.png}"
[ -f "$WP" ] || exit 0

sleep 2   # let xfdesktop create its backdrop properties for the current monitor

xfconf-query -c xfce4-desktop -l 2>/dev/null | grep -E '/last-image$' | while read -r p; do
  xfconf-query -c xfce4-desktop -p "$p" -s "$WP" 2>/dev/null
done
xfconf-query -c xfce4-desktop -l 2>/dev/null | grep -E '/image-style$' | while read -r p; do
  xfconf-query -c xfce4-desktop -p "$p" -s 5 2>/dev/null   # 5 = zoomed (fill screen)
done

# Fallback if no backdrop property existed yet (fresh session).
xfconf-query -c xfce4-desktop -p /backdrop/screen0/monitor0/workspace0/last-image \
  -n -t string -s "$WP" 2>/dev/null

xfdesktop --reload 2>/dev/null || true

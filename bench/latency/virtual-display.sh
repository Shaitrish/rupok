#!/bin/bash
# A headless GNOME compositor with one 4K virtual monitor, for a set time:
# a real GPU swap chain to measure frame rates against without a 4K screen
# (pen-latency.mjs --4k --wayland wl-4k). It is isolated from the desktop
# session: its own session, HOME, config, data and runtime directories and
# no dconf (the memory backend: no user settings, no extensions), so none
# of its helpers can open anything on the real desktop. When the time is up
# everything it started is killed.
#
#   usage: virtual-display.sh DIR SECONDS [WxH@HZ]
#   then:  node pen-latency.mjs --4k --wayland wl-4k --runtime-dir DIR/rt --home DIR/home
#
# DIR must be short: a socket path holds about 100 characters.
set -eu
D="$1"; SECONDS_UP="$2"; MODE="${3:-3840x2160@60}"
RT="$D/rt"; H="$D/home"
mkdir -p "$RT" "$H"
chmod 700 "$RT"
exec setsid env -i HOME="$H" USER="${USER:-$(id -un)}" PATH=/usr/bin:/bin LANG=C.UTF-8 \
  XDG_RUNTIME_DIR="$RT" XDG_CONFIG_HOME="$H/.config" XDG_DATA_HOME="$H/.local/share" XDG_CACHE_HOME="$H/.cache" \
  XDG_DATA_DIRS=/usr/local/share/:/usr/share/ GSETTINGS_BACKEND=memory \
  sh -c 'timeout --kill-after=5 "$0" dbus-run-session -- gnome-shell --headless --no-x11 --wayland-display wl-4k --virtual-monitor "$1" --sm-disable; pkill -KILL -s 0' "$SECONDS_UP" "$MODE"

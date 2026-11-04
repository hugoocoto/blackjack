#!/bin/sh
# Records docs/gameplay.gif: the shuffle, the cut and a few hands, played by
# the game itself (BJ_AUTO). Needs xvfb-run and ImageMagick.
#
# It runs on a virtual display, so the window can't be hidden or throttled,
# and saves every 4th frame of 26 s of play: 390 frames at 15 fps, shown at 2x.
set -e

root=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cd "$root"
cargo build --release
BJ_AUTO=1 BJ_WATCH=1 BJ_MUTE=1 BJ_SHOT_DIR="$tmp" BJ_SHOTS=$(seq -s, 4 4 1560) \
    xvfb-run -a -s "-screen 0 1440x810x24" target/release/hushjack

# The game image is partly transparent where it draws shadows; on screen the
# window's background shows through, so put it under each frame here too.
# GIF delays are in 1/100 s: 7, 7, 6 repeating averages exactly 15 fps.
set --
i=0
for f in "$tmp"/shot_*.png; do
    magick "$f" -background '#0a080e' -alpha remove -alpha off "$f"
    if [ $((i % 3)) = 2 ]; then d=6; else d=7; fi
    set -- "$@" -delay $d "$f"
    i=$((i + 1))
done
magick "$@" -filter point -resize 200% +dither -loop 0 \
    -layers Optimize -layers OptimizeTransparency docs/gameplay.gif
echo "Wrote docs/gameplay.gif ($i frames)"

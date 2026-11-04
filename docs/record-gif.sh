#!/bin/sh
# Records docs/gameplay.gif: the shuffle, the cut and four hands, played by
# the game itself (BJ_AUTO). Needs xvfb-run and ImageMagick.
#
# Seed 229 always deals the same shoe: the auto-player wins three hands in a
# row (the first a blackjack, so the fire lights up) and loses the fourth,
# which ends at frame 1621 (found by the find_gif_seed test in game.rs).
#
# It runs on a virtual display, so the window can't be hidden or throttled,
# and saves every 4th frame of 28.6 s of play: 429 frames at 15 fps, shown at 2x.
set -e

root=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cd "$root"
cargo build --release
SEED=229
BJ_AUTO=1 BJ_WATCH=1 BJ_MUTE=1 BJ_SHOT_DIR="$tmp" BJ_SHOTS=$(seq -s, 4 4 1716) \
    xvfb-run -a -s "-screen 0 1440x810x24" target/release/hushjack --seed $SEED

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
# The lamp breathes and dust drifts, so the whole felt changes a little every
# frame; -fuzz treats changes under 3% as none, which keeps the file small.
magick "$@" -filter point -resize 200% +dither -loop 0 \
    -fuzz 3% -layers Optimize -layers OptimizeTransparency docs/gameplay.gif
echo "Wrote docs/gameplay.gif ($i frames)"

#!/bin/sh
# Records docs/gameplay.gif: the shuffle, the cut and four hands, played by
# the game itself (BJ_AUTO). Needs xvfb-run and ImageMagick.
#
# Seed 229 always deals the same shoe: the auto-player wins three hands in a
# row (the first a blackjack, then "2 IN A ROW!" and "3 IN A ROW!") and loses
# the fourth, which ends at frame 1621 (found by the find_gif_seed test in
# game.rs).
#
# It runs on a virtual display, so the window can't be hidden or throttled,
# and saves every 4th frame of 28.6 s of play: 429 frames at 15 fps. It saves
# the 960x540 window (twice the game's size), screen shader included, so the
# GIF looks like the game; and it uses an empty data folder, so the saved
# theme and records don't show up in it.
set -e

root=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cd "$root"
cargo build --release
SEED=229
XDG_DATA_HOME="$tmp/data" BJ_AUTO=1 BJ_WATCH=1 BJ_MUTE=1 BJ_SHOT_SCREEN=1 BJ_WINDOW=960x540 \
    BJ_SHOT_DIR="$tmp" BJ_SHOTS=$(seq -s, 4 4 1716) \
    xvfb-run -a -s "-screen 0 1440x810x24" target/release/hushjack --seed $SEED

# Make sure no frame is partly transparent (the window background shows
# through on screen).
# GIF delays are in 1/100 s: 7, 7, 6 repeating averages exactly 15 fps.
set --
i=0
for f in "$tmp"/shot_*.png; do
    magick "$f" -background '#0a080e' -alpha remove -alpha off "$f"
    if [ $((i % 3)) = 2 ]; then d=6; else d=7; fi
    set -- "$@" -delay $d "$f"
    i=$((i + 1))
done
# The lamp breathes, dust drifts and the screen has a faint grain, so the
# whole picture changes a little every frame; -fuzz treats changes under 2%
# as none, which keeps the file small.
magick "$@" +dither -loop 0 \
    -fuzz 2% -layers Optimize -layers OptimizeTransparency docs/gameplay.gif
echo "Wrote docs/gameplay.gif ($i frames)"

#!/bin/sh
# Regenerates every icon file from logo.py. Needs python3, rsvg-convert and
# ImageMagick.
set -e
cd "$(dirname "$0")"
python3 -I logo.py > hushjack.svg
rsvg-convert -w 256 -h 256 hushjack.svg -o hushjack.png
# Raw RGBA pixels for the window icon (see main.rs).
for n in 16 32 64; do
    rsvg-convert -w $n -h $n hushjack.svg | magick - -depth 8 rgba:window-$n.rgba
done
# Android launcher icons (see [package.metadata.android] in Cargo.toml).
for d in mdpi:48 hdpi:72 xhdpi:96 xxhdpi:144 xxxhdpi:192; do
    mkdir -p ../../packaging/android/res/mipmap-${d%:*}
    rsvg-convert -w ${d#*:} -h ${d#*:} hushjack.svg -o ../../packaging/android/res/mipmap-${d%:*}/ic_launcher.png
done

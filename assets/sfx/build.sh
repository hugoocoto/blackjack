#!/bin/sh
# Builds the game's sound banks (assets/sfx/<bank>-<n>.ogg) from two Kenney
# packs, both CC0: Casino Audio and Interface Sounds
# (https://kenney.nl/assets/casino-audio, https://kenney.nl/assets/interface-sounds).
# Unpack each and pass its Audio folder:
#
#     assets/sfx/build.sh path/to/casino-audio/Audio path/to/interface-sounds/Audio
#
# Every take is made mono and brought to its bank's loudness (RMS, with the
# peak kept under -1 dBFS), so the volumes in main.rs are the whole mix. Banks
# with few takes also get slightly pitched copies, so repeats sound less
# alike. The shuffle recording is cut into its separate riffles. Needs ffmpeg.
set -e

casino=${1:?casino-audio Audio folder}
ui=${2:?interface-sounds Audio folder}
out=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

rm -f "$out"/*.ogg

# take SRC BANK RMS [PITCH...]: writes BANK-<next>.ogg at RMS dBFS, and one
# more per PITCH factor.
n_of() { ls "$out" | grep -c "^$1-[0-9]*\.ogg$" || true; }
take() {
    src=$1 bank=$2 rms=$3
    shift 3
    stats=$(ffmpeg -hide_banner -nostats -i "$src" -ac 1 \
        -af astats=measure_overall=RMS_level+Peak_level:measure_perchannel=none -f null - 2>&1)
    have=$(echo "$stats" | awk '/RMS level/ {print $NF}')
    peak=$(echo "$stats" | awk '/Peak level/ {print $NF}')
    gain=$(awk -v t="$rms" -v h="$have" -v p="$peak" 'BEGIN {
        g = t - h; if (p + g > -1) g = -1 - p; printf "%.2f", g }')
    for k in 1 "$@"; do
        n=$(($(n_of "$bank") + 1))
        ffmpeg -hide_banner -loglevel error -y -i "$src" -ac 1 \
            -af "asetrate=44100*$k,aresample=44100,volume=${gain}dB" \
            -c:a libvorbis -q:a 4 "$out/$bank-$n.ogg"
    done
}

# The shuffle recording is seven riffles with quiet between them: cut each
# out (a little lead-in, a short fade at the end, the harsh top taken off),
# skipping slivers between two silences that nearly touch.
ffmpeg -hide_banner -nostats -i "$casino/card-shuffle.ogg" \
    -af silencedetect=noise=-35dB:d=0.06 -f null - 2>&1 |
    awk '/silence_end/ {e = $5} /silence_start/ && e != "" {if ($5 - e > 0.1) print e, $5; e = ""}' |
    while read -r from to; do
        i=$(($(ls "$tmp" | wc -l) + 1))
        ffmpeg -hide_banner -loglevel error -y -i "$casino/card-shuffle.ogg" \
            -af "atrim=start=$(awk -v f="$from" 'BEGIN {print f - 0.02}'):end=$(awk -v t="$to" 'BEGIN {print t + 0.06}'),asetpts=PTS-STARTPTS,afade=t=out:st=$(awk -v f="$from" -v t="$to" 'BEGIN {print t - f + 0.02}'):d=0.06,lowpass=f=5000" \
            "$tmp/riffle-$i.wav"
    done
for f in "$tmp"/riffle-*.wav; do take "$f" riffle -22; done

for i in 1 2 3 4; do take "$casino/card-shove-$i.ogg" shove -22 0.94; done
# Softened: the scrape above ~3.5 kHz rolled off, and a quicker tail.
for i in 1 2 3 4 5 6 7 8; do
    ffmpeg -hide_banner -loglevel error -y -i "$casino/card-slide-$i.ogg" \
        -af "lowpass=f=3500:p=2,afade=t=out:st=0.3:d=0.2" "$tmp/slide-$i.wav"
    take "$tmp/slide-$i.wav" slide -24
done
for i in 1 2 3 4; do take "$casino/card-place-$i.ogg" place -21 0.95 1.05; done
for i in 1 2; do take "$casino/card-fan-$i.ogg" fan -20 0.94; done
for i in 1 2 3; do take "$casino/chip-lay-$i.ogg" chip -19; done
for i in 1 2 3 4; do take "$casino/chips-collide-$i.ogg" chip -19; done
for i in 1 2 3 4 5 6; do take "$casino/chips-stack-$i.ogg" payout -18; done
for i in 1 2 3 4 5 6; do take "$casino/chips-handle-$i.ogg" sweep -22; done
# The button clicks and the chart/theme toggles come in three styles each,
# chosen in the theme picker; each is the start of a table sound, cut short.
# clip SRC LEN [FILTER]: the first LEN seconds from where SRC's sound starts,
# fading out, into $tmp/clip.wav.
clip() {
    ffmpeg -hide_banner -loglevel error -y -i "$1" -ac 1 \
        -af "silenceremove=start_periods=1:start_threshold=-40dB,atrim=end=$2,afade=t=out:st=$(awk -v l="$2" 'BEGIN {print l * 0.6}'):d=$(awk -v l="$2" 'BEGIN {print l * 0.4}')${3:+,$3}" \
        "$tmp/clip.wav"
}
for i in 1 2 3; do clip "$casino/chip-lay-$i.ogg" 0.09; take "$tmp/clip.wav" click-chip -22 0.92; done
for i in 1 2 3 4; do clip "$casino/die-throw-$i.ogg" 0.1 lowpass=f=3000; take "$tmp/clip.wav" click-die -22; done
for i in 1 2 3 4; do clip "$casino/chips-collide-$i.ogg" 0.08 lowpass=f=5000; take "$tmp/clip.wav" click-clack -22; done
for i in 1 2; do
    clip "$casino/card-fan-$i.ogg" 0.25 lowpass=f=4500
    take "$tmp/clip.wav" toggle-fan -24 0.94 1.06
done
for i in 1 2 3 4; do clip "$casino/card-shove-$i.ogg" 0.22 lowpass=f=4000; take "$tmp/clip.wav" toggle-shove -24; done
for i in 2 5 7 8; do clip "$casino/card-slide-$i.ogg" 0.2 lowpass=f=3000; take "$tmp/clip.wav" toggle-swish -24; done
for i in 2 3 5 6; do take "$ui/glass_00$i.ogg" chime -22; done

ls "$out"/*.ogg | sed 's/.*\///; s/-[0-9]*\.ogg//' | sort | uniq -c

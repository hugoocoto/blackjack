"""Composes the game's music, synthesized from scratch: a 32-bar lo-fi
jazz tune in AABA form (like most standards), as stems that loop seamlessly
and line up sample for sample, so the game can fade them with the run (see
Music in main.rs):

    table   Rhodes comping and the bass, in two   always
    groove  brushes and a soft kick, laid back    always; more when winning
    lead    the vibraphone theme                  a winning run
    swing   a ride cymbal and busier brushes      a long winning run
    cold    Satie-like: a low note, a soft chord, a note of the theme a bar
                                                  a losing run

The game decodes every sound whole (about 350 KB a second), so only the
stems that follow the chords are the tune's full length; the drums are
4-bar loops, which stay in step with it since 32 is a multiple of 4.

How it's made, and why:

- About 72 BPM (71.8, see LOOP4), a resting heart rate: slow background music makes people
  settle and stay (Milliman's shop and restaurant studies). Light swing,
  drums a little behind the beat (the Dilla / Nujabes lean).
- AABA, 32 bars (~1:47): long enough not to be noticed as a loop. The A
  sections step down the scale (Fmaj9 Em9 Dm9 Cmaj9, the lo-fi "sigh"),
  the bridge borrows from a semitone away (Bb, Ab, Db), and every section
  ends on a ii-V into the next.
- Rootless voicings, each chord moved to the nearest voicing of the last
  (Bill Evans' smooth voice leading).
- The theme is composed: a two-bar motif, a falling maj7 arpeggio, then
  sequenced a step down, call and response, the top of the phrase near
  its end. It only plays when winning, so it becomes the reward.
- Winning harder doesn't speed the tempo; the ride comes in over busier
  brushes, which is how a band lifts the feel.
- Cards and chips sound at 2-6 kHz; the music keeps out of their way.

Run from the repository root (needs numpy and ffmpeg):

    python3 assets/music/compose.py [--preview DIR]

--preview also writes a few mixes of the stems into DIR, to listen to.
Everything is seeded: the same script always writes the same music.
"""

import itertools
import os
import subprocess
import sys
import tempfile

import numpy as np

SR = 44100
BARS = 32
# The drum loops are 4 bars, an exact eighth of the tune. Every loop is a
# whole number of 4096-frame blocks, the audio buffer the game's mixer uses
# on Linux and macOS (1024 on Android): unpatched, it cuts a looping sound
# that wraps inside a buffer (see patches/README.md). So the tempo is the one
# nearest 72 BPM that makes it so.
LOOP4 = round(16 * 60 / 72 * SR / 4096) * 4096
LOOP = 8 * LOOP4
BEAT = LOOP4 / (16 * SR)
BPM = 60 / BEAT
SWING = 0.6  # where the off-beat eighth falls within the beat
OUT = os.path.dirname(os.path.abspath(__file__))

rng = np.random.default_rng(229)


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


def at(beat, late=0.0):
    """Sample index of a beat from the start of the loop, `late` seconds
    behind it, with a touch of human unevenness."""
    return round((beat * BEAT + late + rng.normal(0, 0.004)) * SR)


def place(buf, sound, start):
    """Adds `sound` into the loop `buf` at `start`, wrapping round its end."""
    idx = (np.arange(len(sound)) + start) % len(buf)
    np.add.at(buf, idx, sound)


def shape(x, f_gain):
    """Filters round the loop (circular, so the seam stays seamless):
    `f_gain(f)` is the gain at each frequency."""
    f = np.fft.rfftfreq(len(x), 1 / SR)
    return np.fft.irfft(np.fft.rfft(x) * f_gain(f), len(x))


def lowpass(x, fc, order=2):
    return shape(x, lambda f: 1 / np.sqrt(1 + (f / fc) ** (2 * order)))


def highpass(x, fc, order=2):
    return shape(x, lambda f: 1 / np.sqrt(1 + (np.maximum(f, 1e-9) / fc) ** (-2 * order)))


def make_room(x):
    """A gentle dip where cards and chips sound (2-6 kHz)."""
    return shape(x, lambda f: 1 - 0.45 * np.exp(-((np.log2(np.maximum(f, 1) / 3500)) ** 2) / 0.5))


def env(n, attack, tau, release_at=None, release=0.12):
    t = np.arange(n) / SR
    e = np.minimum(t / attack, 1) * np.exp(-t / tau)
    if release_at is not None:
        r = min(round(release_at * SR), n)
        e[r:] *= np.exp(-(t[r:] - release_at) / release)
    return e


def swing(beat):
    """Moves off-beat eighths (x.5) to the swung position."""
    whole = np.floor(beat)
    return whole + (SWING if abs(beat - whole - 0.5) < 1e-6 else beat - whole)


# ------------------------------------------------------------------ harmony

# Chord tones of each quality, rootless (semitones above the root): what a
# jazz pianist plays while the bass has the root.
QUALITY = {
    "maj9": [4, 7, 11, 14],
    "m9": [3, 7, 10, 14],
    "13": [4, 10, 14, 21],
    "13sus": [5, 10, 14, 21],
    "7b9": [4, 10, 13, 21],
    "maj7#11": [4, 11, 14, 18],
}
NOTE = {"C": 0, "Db": 1, "D": 2, "Eb": 3, "E": 4, "F": 5, "Gb": 6, "G": 7, "Ab": 8, "A": 9, "Bb": 10, "B": 11}


def chord(name):
    root = next(r for r in sorted(NOTE, key=len, reverse=True) if name.startswith(r))
    return NOTE[root], QUALITY[name[len(root):]]


# Two chords a bar ("A B") or one. Each A ends on a ii-V into what follows.
A1 = ["Fmaj9", "Em9", "Dm9", "Cmaj9", "Fmaj9", "Em9 A7b9", "Dm9", "Gm9 C13"]
A2 = ["Fmaj9", "Em9", "Dm9", "Cmaj9", "Fmaj9", "E7b9", "Am9 D13", "Cm9 F13"]
B = ["Bbmaj9", "Am9", "Abmaj9", "G13sus G13", "Ebmaj9", "Dm9", "Dbmaj7#11", "Gm9 C7b9"]
A3 = ["Fmaj9", "Em9", "Dm9", "Cmaj9", "Fmaj9", "Em9 A7b9", "Dm9 G13", "Cmaj9 C13"]
FORM = A1 + A2 + B + A3

# (beat, chord) for every chord change, in beats from the start.
CHANGES = []
for bar, names in enumerate(FORM):
    names = names.split()
    for i, n in enumerate(names):
        CHANGES.append((bar * 4 + i * 4 / len(names), chord(n)))


def chord_at(beat):
    return [c for b, c in CHANGES if b <= beat % (BARS * 4)][-1]


def voicings():
    """For each change, the voicing nearest the one before (smooth voice
    leading): the chord tones, each in some octave, between F3 and B4,
    no wider than a tenth. Close intervals low down are muddy (the low
    interval limits arrangers keep to), so a semitone below C4, or a tone
    below A3, counts against a voicing."""
    out, prev = [], [57, 60, 64, 67]
    for _, (root, tones) in CHANGES:
        pcs = [(root + t) % 12 for t in tones]
        best = None
        for octs in itertools.product(*[[pc + 12 * o for o in range(3, 7) if 53 <= pc + 12 * o <= 71] for pc in pcs]):
            v = sorted(octs)
            if v[-1] - v[0] > 16 or len(set(v)) < 4:
                continue
            mud = sum(8 for lo, hi in zip(v, v[1:]) if (hi - lo == 1 and lo < 60) or (hi - lo == 2 and lo < 57))
            cost = sum(abs(a - b) for a, b in zip(v, prev)) + mud
            if best is None or cost < best[0]:
                best = (cost, v)
        assert best, f"no voicing for {pcs}"
        out.append(best[1])
        prev = best[1]
    return out


VOICINGS = voicings()

# The theme: (beat in bar, MIDI note, length in beats) for each bar.
THEME_A = [
    [(0.5, 76, 0.5), (1, 72, 0.5), (1.5, 69, 1.5)],  # Fmaj9: E C A, a falling maj7
    [(0.5, 74, 0.5), (1, 71, 0.5), (1.5, 67, 1.5)],  # Em9: the same, a step down
    [(0.5, 72, 0.5), (1, 69, 0.5), (1.5, 65, 0.5), (2, 64, 2)],  # Dm9: once more, and it lands
    [(2.5, 67, 0.5), (3, 71, 0.5), (3.5, 74, 0.5)],  # Cmaj9: a pickup, rising
    [(0, 76, 1.5), (1.5, 79, 0.5), (2, 76, 0.5), (2.5, 72, 1.5)],  # Fmaj9: answer, reaching up
]
END_A1 = [
    [(0, 74, 1), (1, 71, 1), (2, 70, 0.5), (2.5, 73, 1.5)],  # Em9 A7b9
    [(0, 74, 0.5), (0.5, 76, 0.5), (1, 77, 1), (2, 76, 0.5), (2.5, 72, 1.5)],  # Dm9: the top, F
    [(0, 74, 2), (2, 70, 1), (3, 76, 1)],  # Gm9 C13, into the next A
]
END_A2 = [
    [(0, 74, 0.5), (0.5, 71, 0.5), (1, 68, 2)],  # E7b9
    [(0, 72, 1), (1, 71, 1), (2, 69, 0.5), (2.5, 66, 1.5)],  # Am9 D13
    [(0, 75, 1), (1, 74, 1), (2, 72, 0.5), (2.5, 69, 1.5)],  # Cm9 F13, into the bridge
]
THEME_B = [
    [(0, 74, 2), (2, 77, 1), (3, 81, 1)],  # Bbmaj9: longer notes, higher
    [(0, 79, 3), (3, 76, 1)],  # Am9
    [(0, 72, 2), (2, 75, 1), (3, 79, 1)],  # Abmaj9
    [(0, 77, 2), (2, 76, 1), (3, 74, 1)],  # G13sus G13
    [(0, 79, 2), (2, 74, 2)],  # Ebmaj9
    [(0, 77, 1.5), (1.5, 76, 0.5), (2, 72, 2)],  # Dm9
    [(0, 72, 1), (1, 67, 1), (2, 68, 2)],  # Dbmaj7#11
    [(0, 70, 2), (2, 76, 1), (3, 73, 1)],  # Gm9 C7b9, into the last A
]
END_A3 = [
    [(0, 74, 1), (1, 71, 1), (2, 70, 0.5), (2.5, 73, 1.5)],  # Em9 A7b9
    [(0, 74, 0.5), (0.5, 77, 0.5), (1, 76, 1), (2, 74, 1), (3, 71, 1)],  # Dm9 G13
    [(0, 72, 2), (2, 70, 1)],  # Cmaj9 C13: home, and on to the top
]
THEME = THEME_A + END_A1 + THEME_A + END_A2 + THEME_B + THEME_A + END_A3


# ------------------------------------------------------------- instruments

def rhodes(midi, dur, vel=0.5, dark=False):
    """FM electric piano: a sine bent by its own frequency, the bend fading
    fast (the tine's bark, softer when played softly), a faint bell, and a
    slow tremolo."""
    n = round((dur + 1.6) * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    # The lasting bend (0.35) gives the tone overtones at 400-1500 Hz, so it
    # still carries on a phone's speaker, which plays little under 400.
    index = (0.5 if dark else 1.1) * vel * np.exp(-t / 0.25) + 0.35
    tone = np.sin(2 * np.pi * f * t + index * np.sin(2 * np.pi * f * t))
    if not dark:
        tone += 0.05 * vel * np.sin(2 * np.pi * f * 7.1 * t) * np.exp(-t / 0.05)
    trem = 1 + 0.1 * np.sin(2 * np.pi * 4.0 * t + rng.uniform(0, 6.3))
    return tone * env(n, 0.005, 1.8, dur, 0.3) * trem * vel


def upright(midi, dur, vel=0.7):
    """Upright bass: a few harmonics plucked, sliding a hair into pitch,
    with a soft thump of the finger. The upper harmonics last, so on a
    phone, which can't play the low root, the ear still hears it from them
    (the missing fundamental)."""
    n = round((dur + 0.4) * SR)
    t = np.arange(n) / SR
    f = hz(midi) * (1 - 0.012 * np.exp(-t / 0.025))
    phase = 2 * np.pi * np.cumsum(f) / SR
    tone = np.sin(phase) + sum(0.6 / k * np.sin(k * phase) * np.exp(-t * k / 2.5) for k in range(2, 9))
    return tone * env(n, 0.008, 1.0, dur, 0.07) * vel


def vibes(midi, dur, vel=0.5):
    """Vibraphone: a pure bar, its bright partial dying fast, the motor's
    tremolo."""
    n = round((dur + 1.4) * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    tone = np.sin(2 * np.pi * f * t) + 0.15 * np.sin(2 * np.pi * f * 4 * t) * np.exp(-t / 0.06)
    trem = 1 + 0.25 * np.sin(2 * np.pi * 5.0 * t)
    return tone * env(n, 0.003, 1.3, dur, 0.5) * trem * vel


def brush(vel, attack=0.012, tau=0.11):
    n = round(0.4 * SR)
    return rng.standard_normal(n) * env(n, attack, tau) * vel


def kick(vel):
    n = round(0.4 * SR)
    t = np.arange(n) / SR
    f = 50 + 50 * np.exp(-t / 0.025)
    return np.sin(2 * np.pi * np.cumsum(f) / SR) * env(n, 0.003, 0.14) * vel


def ride(vel):
    """A dark ride cymbal, played soft: inharmonic partials ringing over a
    little noise, the higher ones quieter and shorter, so it shimmers rather
    than hisses (small speakers are loudest at 2-5 kHz)."""
    n = round(1.2 * SR)
    t = np.arange(n) / SR
    x = sum(
        a * np.sin(2 * np.pi * f * t + rng.uniform(0, 6.3)) * np.exp(-t * f / 6000)
        for f, a in ((1870, 0.8), (2950, 1), (3710, 0.6), (4420, 0.35), (5390, 0.2))
    )
    x = x / 3 + 0.12 * rng.standard_normal(n) * np.exp(-t / 0.08)
    return x * env(n, 0.004, 0.35) * vel


def crackle(level):
    """Vinyl: sparse, small clicks over a faint, dark hiss."""
    x = rng.standard_normal(LOOP) * 0.012
    clicks = rng.random(LOOP) < 3 / SR
    x[clicks] += rng.standard_normal(clicks.sum()) * 0.35
    return lowpass(x, 3000) * level


# ------------------------------------------------------------------ parts

# Comping rhythms for one chord of a whole bar: (beat, length, velocity).
# Off-beat stabs and a push on the "and of 4" into the next chord, like
# Red Garland or Wynton Kelly behind a soloist.
COMP_BAR = [
    [(0, 1.5, 0.5), (2.5, 1.3, 0.4)],
    [(0, 2.5, 0.5), (3.5, 0.5, 0.3)],
    [(0.5, 1.0, 0.4), (2, 1.6, 0.45)],
    [(0, 0.9, 0.45), (1.5, 0.9, 0.35), (3, 0.8, 0.35)],
]
COMP_HALF = [[(0, 1.5, 0.45)], [(0, 0.9, 0.45), (1.5, 0.4, 0.3)]]


def low(pc):
    """A pitch class on the bass, between E1 and D#2."""
    return 28 + (pc - 4) % 12


def keys():
    buf = np.zeros(LOOP)
    for i, (beat0, _) in enumerate(CHANGES):
        span = (CHANGES[(i + 1) % len(CHANGES)][0] - beat0) % (BARS * 4) or BARS * 4
        rhythms = COMP_BAR if span >= 4 else COMP_HALF
        for beat, length, vel in rhythms[rng.integers(len(rhythms))]:
            for k, m in enumerate(VOICINGS[i]):
                start = at(beat0 + swing(beat)) + round(k * 0.011 * SR)
                place(buf, rhodes(m, length * BEAT, vel * rng.uniform(0.85, 1)), start)
    return make_room(lowpass(buf, 2600)) + crackle(0.45)


def bass():
    """In two: the root, then the fifth, and a step into the next chord."""
    buf = np.zeros(LOOP)
    for i, (beat0, (root, _)) in enumerate(CHANGES):
        span = (CHANGES[(i + 1) % len(CHANGES)][0] - beat0) % (BARS * 4) or BARS * 4
        r = low(root)
        nxt = low(CHANGES[(i + 1) % len(CHANGES)][1][0])
        if span >= 4:
            notes = [(0, r, 1.8, 0.75), (2, r + 7, 1.2, 0.6), (3.5, nxt + rng.choice([-1, 1]), 0.4, 0.45)]
        else:
            notes = [(0, r, 1.8, 0.7)]
        for beat, m, length, vel in notes:
            place(buf, upright(m, length * BEAT, vel), at(beat0 + swing(beat), late=0.008))
    return lowpass(buf, 900)


def part_table():
    k, b = keys(), bass()
    return k / np.sqrt(np.mean(k**2)) * 0.085 + b / np.sqrt(np.mean(b**2)) * 0.07


def part_swing():
    """The ride's ding, ding-a ding, and ghost taps of the brush in
    between: the feel lifts, the tempo doesn't."""
    cym, ghost = np.zeros(LOOP4), np.zeros(LOOP4)
    for bar in range(4):
        for beat, vel in [(0, 0.5), (1, 0.6), (1.5, 0.35), (2, 0.5), (3, 0.6), (3.5, 0.35)]:
            place(cym, ride(vel), at(bar * 4 + swing(beat)))
        for beat in (0.5, 1.75, 2.5, 3.75):
            place(ghost, brush(0.18, tau=0.05), at(bar * 4 + beat, late=0.012))
    return make_room(lowpass(highpass(cym, 1200), 4500)) + make_room(lowpass(highpass(ghost, 700), 3000))


def part_groove():
    """Brushes on 2 and 4 a little late, a soft sweep on the eighths, and a
    kick that mostly just marks the one."""
    snare, sweep, kicks = np.zeros(LOOP4), np.zeros(LOOP4), np.zeros(LOOP4)
    for bar in range(4):
        b0 = bar * 4
        for beat in (1, 3):
            place(snare, brush(0.5), at(b0 + beat, late=0.018))
        for beat in np.arange(0, 4, 0.5):
            place(sweep, brush(0.08, attack=0.04), at(b0 + swing(beat)))
        for beat in ([0, 2] if bar != 3 else [0, 1.5, 2]):
            place(kicks, kick(0.65), at(b0 + swing(beat)))
    return (
        make_room(lowpass(highpass(snare, 600), 3500))
        + lowpass(highpass(sweep, 500), 2200)
        + lowpass(kicks, 150)
    )


def part_lead():
    buf = np.zeros(LOOP)
    for bar, notes in enumerate(THEME):
        for beat, m, length in notes:
            vel = 0.5 * rng.uniform(0.85, 1) * (1.1 if beat == 0 else 1)
            place(buf, vibes(m, length * BEAT, vel), at(bar * 4 + swing(beat), late=0.01))
    return make_room(lowpass(buf, 3500))


def part_cold():
    """Gymnopedie-like: a low root on the one, a soft chord after it, and
    the first note of the theme's bar, far away. Lots of room."""
    buf = np.zeros(LOOP)
    for i, (beat0, (root, _)) in enumerate(CHANGES):
        place(buf, 0.8 * upright(low(root) + 12, 2.5 * BEAT, 0.5), at(beat0))
        for k, m in enumerate(VOICINGS[i][1:]):
            place(buf, rhodes(m, 2 * BEAT, 0.3, dark=True), at(beat0 + 1) + round(k * 0.03 * SR))
    for bar, notes in enumerate(THEME):
        if notes:
            _, m, _ = notes[0]
            place(buf, 0.6 * rhodes(m, 3 * BEAT, 0.35, dark=True), at(bar * 4 + 2))
    return lowpass(buf, 1500) + crackle(0.45)


# ------------------------------------------------------------------- output

STEMS = {
    "table": part_table,
    "groove": part_groove,
    "lead": part_lead,
    "swing": part_swing,
    "cold": part_cold,
}
# Each stem's loudness (RMS) in the mix, so the game fades each 0..1.
LEVELS = {"table": 0.11, "groove": 0.045, "lead": 0.06, "swing": 0.02, "cold": 0.1}


def write(path, x):
    with tempfile.NamedTemporaryFile(suffix=".f32") as raw:
        raw.write(x.astype(np.float32).tobytes())
        raw.flush()
        subprocess.run(
            ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "f32le", "-ar", str(SR), "-ac", "1",
             "-i", raw.name, "-c:a", "libvorbis", "-q:a", "3", path],
            check=True,
        )


def main():
    preview = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == "--preview" else None
    for old in os.listdir(OUT):
        if old.endswith(".ogg"):
            os.remove(os.path.join(OUT, old))
    stems = {}
    for name, part in STEMS.items():
        x = part()
        stems[name] = x / np.sqrt(np.mean(x**2)) * LEVELS[name]
    # The loudest mix the game makes (everything but cold) must not clip.
    full = lambda x: np.tile(x, LOOP // len(x))  # noqa: E731
    loudest = np.abs(sum(full(x) for k, x in stems.items() if k != "cold")).max()
    scale = min(1, 0.9 / loudest)
    for name, x in stems.items():
        write(os.path.join(OUT, f"{name}.ogg"), x * scale)
        print(f"{name}.ogg  {len(x) / SR:.1f}s  peak {np.abs(x * scale).max():.2f}")
    if preview:
        os.makedirs(preview, exist_ok=True)
        for old in os.listdir(preview):
            os.remove(os.path.join(preview, old))
        mixes = {
            "1-calm": {"table": 1, "groove": 0.55},
            "2-winning": {"table": 1, "groove": 1, "lead": 1},
            "3-long-streak": {"table": 1, "groove": 1, "lead": 1, "swing": 1},
            "4-losing": {"cold": 1},
        }
        for name, mix in mixes.items():
            x = sum(full(stems[k]) * v for k, v in mix.items()) * scale
            write(os.path.join(preview, f"{name}.ogg"), x)
            print(f"preview {name}.ogg")


if __name__ == "__main__":
    main()

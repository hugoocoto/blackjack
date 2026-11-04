# Patched dependencies

`quad-snd/` is quad-snd 0.2.8 from crates.io with one fix in
`src/mixer.rs` (marked "Hushjack patch"): when a looping sound wraps in the
middle of an audio buffer, the mixer wrote its start over its end and left the
rest of the buffer silent, which cut the music at every loop. Drop the patch
once a fixed quad-snd is released.

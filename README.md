# Hushjack

<img src="assets/icon/hushjack.png" alt="" width="96" align="right">

Pixel-art blackjack for one player.

![Gameplay](docs/gameplay.gif)

## Play

- **Browser:** <https://hugoocoto.github.io/blackjack/>
- **Windows, macOS, Linux, Android:** download from the
  [releases page](https://github.com/hugoocoto/blackjack/releases).
- **Cargo:** `cargo install hushjack` (Linux needs the ALSA dev package:
  `libasound2-dev`, `alsa-lib-devel` or `alsa-lib`).

The builds aren't signed. On Windows, click *More info → Run anyway*. On
macOS, run `xattr -d com.apple.quarantine hushjack`. On Linux,
`./install.sh` (or `hushjack --install-desktop` for the cargo build) adds a
menu entry.

## Controls

| Key | Action |
| --- | --- |
| `Space` | Deal / stand / skip |
| `H` `S` `D` `P` | Hit, stand, double, split |
| `I` / `N` | Take or decline insurance |
| `←` `→` / wheel | Change your bet |
| `C` | Strategy chart |
| `T` | Themes: cards, felt, sounds and music |
| `M` | Music on or off |
| `B` | Next card back |
| `F` | Fullscreen |
| `Esc` | Records board; again to quit (new table in the browser) |

Everything is clickable too.

To add your own themes, put a `themes.txt` next to the saved records
(`~/.local/share/hushjack`, `~/Library/Application Support/hushjack` or
`%APPDATA%\hushjack`), one look per line, colours as `R,G,B`:

```text
front SEPIA paper=240,228,200 trim=214,194,156 red=150,40,30 black=60,40,30
back OCEAN fill=20,60,90 accent=220,220,200 suit=D
felt TEAL cloth=20,96,104
```

Fronts also take `hearts=`, `diamonds=`, `clubs=` and `spades=`; backs
`weave=`; felts `line=` and `print=`. Anything left out gets a default.

## Rules

6 decks, blackjack pays 3:2, dealer stands on all 17s and peeks. Double on
any two cards, also after splitting; split up to 4 hands; split aces get one
card. Insurance pays 2:1. No surrender. The shoe is reshuffled when the cut
card comes out.

## Build

```sh
cargo run --release
```

## Credits

Cards, sounds and font by [Kenney](https://kenney.nl) (CC0, see
`assets/licenses/`). The music is composed in code
(`assets/music/compose.py`). Code under the GPL v3 or later, see [LICENSE](LICENSE).

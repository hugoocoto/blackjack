# Blackjack

A small pixel-art blackjack table for one player, written in Rust. It opens
straight into the game. A dealer hand-shuffles 6 decks the way a casino does,
you cut, and you play from the shoe for as long as you like.

![Gameplay](docs/gameplay.gif)

## Play

```sh
cargo run --release
```

| Key | Action |
| --- | --- |
| `Space` | Deal, stand, decline insurance, skip the shuffle |
| `H` `S` `D` `P` | Hit, stand, double, split |
| `I` / `N` | Take or decline insurance |
| `←` `→` or mouse wheel | Change your bet |
| `C` | Show or hide the strategy chart |
| `F` | Fullscreen |
| `Esc` | Quit |

You can also click every button. To cut, move the cut card with the mouse
or the arrow keys, then click.

## Rules

- 6 decks. Blackjack pays 3:2. The dealer stands on all 17s and peeks for
  blackjack.
- You can double on any two cards, including after a split. You can split
  up to 4 hands. Split aces get one card each.
- Insurance is offered when the dealer shows an Ace. It costs half your bet
  and pays 2:1.
- There is no surrender.
- You start with 1000 credits. Your balance can go negative, and the game
  never ends.
- The dealer's cut card sits about one deck from the end. When it comes
  out, that hand is the last one, and then the dealer shuffles again.

The strategy chart (`C`) shows basic strategy for these rules. During your
turn, it marks the play for your hand.

## Requirements

Rust (stable). On Linux, sound needs ALSA.

## Credits

The cards, sounds and font are by [Kenney](https://kenney.nl), under the CC0
license. The license files are in `assets/licenses/`.

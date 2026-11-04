#!/bin/sh
# Installs Hushjack for the current user: the game in ~/.local/bin, plus a
# menu entry and icon. No root needed.
#
#   ./install.sh               install
#   ./install.sh --uninstall   remove it again
#
# PREFIX=/some/dir changes where it goes (default ~/.local).
#
# Works from an unpacked release (the game next to this script) and from the
# source tree after `cargo build --release`.
set -e

here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
bin=$prefix/bin/hushjack
# The game writes the menu entry and icon under $XDG_DATA_HOME.
export XDG_DATA_HOME="$prefix/share"

if [ "$1" = --uninstall ]; then
    if [ -x "$bin" ]; then
        "$bin" --uninstall-desktop
    fi
    rm -f "$bin"
    echo "Hushjack removed from $prefix"
    exit
fi

for f in "$here/hushjack" "$here/../../target/release/hushjack"; do
    if [ -f "$f" ]; then
        src=$f
        break
    fi
done
if [ -z "$src" ]; then
    echo "install.sh: can't find hushjack (from source, run 'cargo build --release' first)" >&2
    exit 1
fi

install -Dm755 "$src" "$bin"
"$bin" --install-desktop

echo "Hushjack installed in $prefix"
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) echo "Note: $prefix/bin isn't on your PATH; start it from the menu or add it." ;;
esac

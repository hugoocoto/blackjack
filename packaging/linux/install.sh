#!/bin/sh
# Installs Hushjack for the current user: the game in ~/.local/bin, plus a
# menu entry and icon. No root needed.
#
#   ./install.sh               install
#   ./install.sh --uninstall   remove it again
#
# PREFIX=/some/dir changes where it goes (default ~/.local).
#
# Works from an unpacked release (files next to this script) and from the
# source tree after `cargo build --release`.
set -e

here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
bin_dir=$prefix/bin
apps_dir=$prefix/share/applications
icons_dir=$prefix/share/icons/hicolor

# First of the given paths (relative to this script) that exists.
pick() {
    for f in "$@"; do
        if [ -f "$here/$f" ]; then
            echo "$here/$f"
            return
        fi
    done
    echo "install.sh: can't find $1 (from source, run 'cargo build --release' first)" >&2
    return 1
}

refresh() {
    update-desktop-database -q "$apps_dir" 2>/dev/null || true
    # Only refresh an existing cache: creating one would hide icons that
    # other apps add later without updating it.
    if [ -f "$icons_dir/icon-theme.cache" ]; then
        gtk-update-icon-cache -q "$icons_dir" 2>/dev/null || true
    fi
}

if [ "$1" = --uninstall ]; then
    rm -f "$bin_dir/hushjack" \
        "$apps_dir/hushjack.desktop" \
        "$icons_dir/scalable/apps/hushjack.svg" \
        "$icons_dir/256x256/apps/hushjack.png"
    refresh
    echo "Hushjack removed from $prefix"
    exit
fi

bin=$(pick hushjack ../../target/release/hushjack)
desktop=$(pick hushjack.desktop)
svg=$(pick hushjack.svg ../../assets/icon/hushjack.svg)
png=$(pick hushjack.png ../../assets/icon/hushjack.png)

install -Dm755 "$bin" "$bin_dir/hushjack"
install -Dm644 "$svg" "$icons_dir/scalable/apps/hushjack.svg"
install -Dm644 "$png" "$icons_dir/256x256/apps/hushjack.png"
# The menu may not have ~/.local/bin on its PATH, so point at the game directly.
mkdir -p "$apps_dir"
sed "s|^Exec=hushjack$|Exec=\"$bin_dir/hushjack\"|" "$desktop" > "$apps_dir/hushjack.desktop"
refresh

echo "Hushjack installed in $prefix"
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "Note: $bin_dir isn't on your PATH; start it from the menu or add it." ;;
esac

"""Draws the 32x32 Hushjack logo and writes it as an SVG of pixel rects.

Run build.sh in this folder after changing it."""
import sys

N = 32
P = {
    "K": "#1a1626",  # outline
    "G": "#2e7a54",  # felt
    "g": "#22603f",  # felt edge
    "h": "#3a8c63",  # felt light
    "W": "#f2eee4",  # card face
    "w": "#c9c4b8",  # card shade
    "S": "#1e1e2a",  # spade
}
px: list[list[str | None]] = [[None] * N for _ in range(N)]


def put(x, y, c):
    if 0 <= x < N and 0 <= y < N:
        px[y][x] = c


def rect(x0, y0, x1, y1, c):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            put(x, y, c)


def sprite(x0, y0, rows):
    for dy, row in enumerate(rows):
        for dx, c in enumerate(row):
            if c != ".":
                put(x0 + dx, y0 + dy, c)


# Felt tile with rounded corners.
r = 5
for y in range(N):
    for x in range(N):
        cx = min(max(x, r), N - 1 - r)
        cy = min(max(y, r), N - 1 - r)
        d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
        if d <= r - 0.3:
            edge = d > r - 1.3 or x in (0, N - 1) and r <= y < N - r or y in (0, N - 1) and r <= x < N - r
            put(x, y, "K" if edge else "G")
for y in range(1, N - 1):
    for x in range(1, N - 1):
        if px[y][x] == "G" and any(px[y + dy][x + dx] == "K" for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
            px[y][x] = "g"
# A soft light across the top of the felt.
for y in range(2, 6):
    for x in range(2, N - 2):
        if px[y][x] == "G" and (x + y) % 2 == 0 and y < 4:
            px[y][x] = "h"


def card(x0, y0, x1, y1):
    rect(x0, y0, x1, y1, "K")
    for (x, y) in ((x0, y0), (x1, y0), (x0, y1), (x1, y1)):
        put(x, y, "G")
    rect(x0 + 1, y0 + 1, x1 - 1, y1 - 1, "W")
    rect(x1 - 1, y0 + 2, x1 - 1, y1 - 1, "w")
    rect(x0 + 2, y1 - 1, x1 - 1, y1 - 1, "w")


# The ace of spades in the middle, with its index in two corners.
card(8, 5, 23, 26)
sprite(10, 7, [".S.", "S.S", "SSS", "S.S", "S.S"])
sprite(18, 19, ["S.S", "S.S", "SSS", "S.S", ".S."])
sprite(12, 11, [
    "...S...",
    "..SSS..",
    ".SSSSS.",
    "SSSSSSS",
    "SSSSSSS",
    "SS.S.SS",
    "...S...",
    "..SSS..",
])

if "--text" in sys.argv:
    for row in px:
        print("".join(c or "." for c in row))
    sys.exit()

out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {N} {N}" width="512" height="512" shape-rendering="crispEdges">']
for y, row in enumerate(px):
    x = 0
    while x < N:
        c = row[x]
        if c is None:
            x += 1
            continue
        run = 1
        while x + run < N and row[x + run] == c:
            run += 1
        out.append(f'<rect x="{x}" y="{y}" width="{run}" height="1" fill="{P[c]}"/>')
        x += run
out.append("</svg>")
print("\n".join(out))

#!/usr/bin/env python3
"""Draws Seer's icon and writes every file made from it.

The mark is சீ (from சீர்: prune, order, neatness) lit up in a mosaic of dim
cells that runs edge to edge across a rounded tile. The letter's two
crossbars take the video and audio data hues, the way Seer's table shows a
file's streams. Two drawings of the letter exist: 12 rows for the app icon
and anything 48px or larger, and 9 rows for small sizes, where 12 rows blur.

    python3 scripts/icon.py

writes, from the letter grids and colours below:

    app-icon.png            1024px macOS app icon, 12-row letter
    app-icon-small.png      256px macOS app icon, 9-row letter (16/32pt)
    site/src/seer.svg       favicon; follows the system light/dark setting
    site/src/seer-light.svg site logo on the light page
    site/src/seer-dark.svg  site logo on the dark page

Only the standard library is used: the PNGs are rasterised here from
rounded-rectangle distance fields, so no SVG renderer is needed.
"""
import array
import math
import pathlib
import struct
import zlib

ROOT = pathlib.Path(__file__).resolve().parent.parent

# '#' letter, 'v' video crossbar, 'a' audio crossbar, '.' background.
# Sampled from the letter's outline onto the grid, then cleaned by hand.
LETTER_12 = [
    ".....####",
    ".....##.#",
    "....#####",
    ".....#...",
    "..####vv.",
    "..#..#...",
    "..#..#...",
    ".#####aa.",
    "##...#...",
    "#....#...",
    "##..##...",
    ".####....",
]
LETTER_9 = [
    "....##.",
    "...#..#",
    "...#.#.",
    ".###vvv",
    ".#.#...",
    "####aaa",
    "#..#...",
    "#..#...",
    ".##....",
]

# Colours: the data hues are DESIGN.md's data_video and data_audio.
APP = dict(  # the macOS icon is always the dark tile
    tile_top=(0x2A, 0x2C, 0x31), tile_bottom=(0x11, 0x12, 0x15),
    ink=(0xF1, 0xF1, 0xEF), video=(0x3F, 0xB8, 0xAF), audio=(0xE0, 0xA3, 0x3E),
    dim=(0xF1, 0xF1, 0xEF), dim_alpha=.07,
)
WEB = {
    "light": dict(tile="#FFFFFF", ring="#000000", ring_alpha=.08, ink="#17181A",
                  video="#0E756E", audio="#A86B0B", dim="#17181A", dim_alpha=.11),
    "dark": dict(tile="#202125", ring="#FFFFFF", ring_alpha=.08, ink="#F1F1EF",
                 video="#3FB8AF", audio="#E0A33E", dim="#F1F1EF", dim_alpha=.11),
}

TILE_RADIUS = 22.5  # % of the tile; macOS's icon corner
GAP = .16           # gap between cells, as a share of a cell
CELL_RADIUS = .26   # cell corner, as a share of a cell


def cells(letter, height):
    """The mosaic for one tile, in tile units (0-100).

    `height` is the share of the tile the letter's rows span. The letter is
    centred and the background grid continues past every edge of the tile,
    so the tile's own mask crops it.
    """
    rows, cols = len(letter), len(letter[0])
    size = height / (rows + (rows - 1) * GAP)
    pitch = size * (1 + GAP)
    x0 = 50 - (cols * pitch - size * GAP) / 2
    y0 = 50 - (rows * pitch - size * GAP) / 2
    reach = math.ceil(50 / pitch) + 1
    out = []
    for r in range(-reach, rows + reach):
        for c in range(-reach, cols + reach):
            x, y = x0 + c * pitch, y0 + r * pitch
            if x > 100 or y > 100 or x + size < 0 or y + size < 0:
                continue
            ch = letter[r][c] if 0 <= r < rows and 0 <= c < cols else "."
            kind = {"#": "ink", "v": "video", "a": "audio"}.get(ch, "dim")
            out.append((x, y, size, kind))
    return out


# ---------- SVG ----------

def svg(letter, height, palettes, adaptive=False):
    """A tile as SVG. With `adaptive`, colours follow prefers-color-scheme."""
    def style(p):
        return (f".t{{fill:{p['tile']}}}.r{{stroke:{p['ring']};stroke-opacity:{p['ring_alpha']}}}"
                f".i{{fill:{p['ink']}}}.v{{fill:{p['video']}}}.a{{fill:{p['audio']}}}"
                f".d{{fill:{p['dim']};fill-opacity:{p['dim_alpha']}}}")

    css = style(palettes["light"])
    if adaptive:
        css += f"@media (prefers-color-scheme:dark){{{style(palettes['dark'])}}}"
    cls = {"ink": "i", "video": "v", "audio": "a", "dim": "d"}
    rects = "".join(
        f'<rect class="{cls[k]}" x="{x:.2f}" y="{y:.2f}" width="{s:.2f}" height="{s:.2f}" rx="{s * CELL_RADIUS:.2f}"/>'
        for x, y, s, k in cells(letter, height)
    )
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">'
        f"<style>{css}</style>"
        f'<clipPath id="tile"><rect width="100" height="100" rx="{TILE_RADIUS}"/></clipPath>'
        f'<rect class="t" width="100" height="100" rx="{TILE_RADIUS}"/>'
        f'<g clip-path="url(#tile)">{rects}</g>'
        f'<rect class="r" x=".75" y=".75" width="98.5" height="98.5" rx="{TILE_RADIUS - .75}" fill="none" stroke-width="1.5"/>'
        "</svg>\n"
    )


# ---------- PNG ----------

def rrect(px, py, x, y, w, h, r):
    """Signed distance from a point to a rounded rectangle (negative inside)."""
    qx = abs(px - (x + w / 2)) - w / 2 + r
    qy = abs(py - (y + h / 2)) - h / 2 + r
    return math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - r


def clamp(v):
    return 0.0 if v < 0 else 1.0 if v > 1 else v


class Canvas:
    """Premultiplied RGBA floats."""

    def __init__(self, n):
        self.n = n
        self.px = array.array("d", bytes(8 * 4 * n * n))

    def over(self, x, y, rgb, a):
        if a <= 0:
            return
        i = 4 * (y * self.n + x)
        p, k = self.px, 1 - a
        p[i] = rgb[0] * a + p[i] * k
        p[i + 1] = rgb[1] * a + p[i + 1] * k
        p[i + 2] = rgb[2] * a + p[i + 2] * k
        p[i + 3] = a + p[i + 3] * k

    def png(self, path):
        raw = bytearray()
        for y in range(self.n):
            raw.append(0)
            for x in range(self.n):
                i = 4 * (y * self.n + x)
                a = self.px[i + 3]
                if a > 0:
                    raw += bytes(round(clamp(self.px[i + c] / a) * 255) for c in range(3))
                else:
                    raw += b"\0\0\0"
                raw.append(round(clamp(a) * 255))

        def chunk(tag, data):
            return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))

        header = struct.pack(">IIBBBBB", self.n, self.n, 8, 6, 0, 0, 0)
        path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
                         + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))


def app_icon(letter, height, n, path):
    """The macOS icon: Apple's grid puts an 824/1024 tile in the canvas, with a soft shadow."""
    c = Canvas(n)
    s = n / 1024
    tx, tw = 100 * s, 824 * s
    tr = TILE_RADIUS / 100 * tw
    unit = tw / 100
    blur, drop = 28 * s, 12 * s
    white = (1.0, 1.0, 1.0)
    rgb = lambda t: tuple(v / 255 for v in t)
    top, bottom = rgb(APP["tile_top"]), rgb(APP["tile_bottom"])

    for y in range(n):
        py = y + .5
        t = clamp((py - tx) / tw)
        shade = tuple(top[k] + (bottom[k] - top[k]) * t for k in range(3))
        for x in range(n):
            px = x + .5
            ds = rrect(px, py - drop, tx, tx, tw, tw, tr)
            if ds < blur:
                c.over(x, y, (0, 0, 0), .32 * clamp(1 - (ds + blur) / (2 * blur)))
            cov = clamp(.5 - rrect(px, py, tx, tx, tw, tw, tr))
            if cov:
                c.over(x, y, shade, cov)
                if t < .5:
                    c.over(x, y, white, cov * .12 * (1 - t / .5))

    colour = {"ink": rgb(APP["ink"]), "video": rgb(APP["video"]), "audio": rgb(APP["audio"]), "dim": rgb(APP["dim"])}
    for x, y, size, kind in cells(letter, height):
        cx, cy, cs = tx + x * unit, tx + y * unit, size * unit
        cr = cs * CELL_RADIUS
        lit = kind != "dim"
        alpha = 1.0 if lit else APP["dim_alpha"]
        for yy in range(max(0, int(cy) - 1), min(n, int(cy + cs) + 2)):
            py = yy + .5
            for xx in range(max(0, int(cx) - 1), min(n, int(cx + cs) + 2)):
                px = xx + .5
                cov = clamp(.5 - rrect(px, py, cx, cy, cs, cs, cr))
                if not cov:
                    continue
                cov *= clamp(.5 - rrect(px, py, tx, tx, tw, tw, tr))
                c.over(xx, yy, colour[kind], cov * alpha)
                if lit:  # a soft highlight across the top of each lit cell
                    f = (py - cy) / cs
                    if f < .6:
                        c.over(xx, yy, white, cov * .3 * (1 - f / .6))
    c.png(path)


def main():
    outputs = {
        ROOT / "site/src/seer.svg": svg(LETTER_9, 84, WEB, adaptive=True),
        ROOT / "site/src/seer-light.svg": svg(LETTER_9, 84, {"light": WEB["light"]}),
        ROOT / "site/src/seer-dark.svg": svg(LETTER_9, 84, {"light": WEB["dark"]}),
    }
    for path, text in outputs.items():
        path.write_text(text)
        print(path.relative_to(ROOT))
    for letter, height, n, name in ((LETTER_12, 76, 1024, "app-icon.png"), (LETTER_9, 84, 256, "app-icon-small.png")):
        app_icon(letter, height, n, ROOT / name)
        print(name)


if __name__ == "__main__":
    main()

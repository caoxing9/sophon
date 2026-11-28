"""Sophon's pet, drawn in pixels: the app's eclipse as a small creature (a
dark disc with a warm corona, two glowing eyes), in four moods the
conversation shows above its composer.

`python3 assets/pet/gen_pet.py` writes one strip per mood next to this file
(`typing.png`, `happy.png`, `waiting.png`, `sleeping.png`): its frames side
by side, each 32 x 32 pixels drawn SCALE times larger so it stays sharp.
Standard library only.
"""

import math
import os
import struct
import zlib

N = 32  # a frame's pixels a side
SCALE = 6
HERE = os.path.dirname(os.path.abspath(__file__))

# The icon's colours.
DISC = (26, 23, 38)
DISC_LIT = (44, 39, 62)
DISC_EDGE = (12, 10, 20)
RIM = (255, 208, 178)
RIM_HOT = (255, 243, 234)
RAY = (255, 154, 104)
RAY_DIM = (184, 98, 70)
EYE = (255, 243, 234)
BLUSH = (255, 128, 112)
KEY = (120, 116, 150)
KEY_LIT = (190, 186, 214)
SCREEN = (60, 56, 86)
CODE = (126, 200, 255)
CODE2 = (196, 150, 255)
CHECK_BG = (52, 182, 104)
WHITE = (255, 255, 255)
BANG_BG = (250, 196, 64)
BANG = (120, 60, 10)
ZZZ = (150, 156, 196)
SPARK = (255, 220, 120)


class Frame:
    def __init__(self):
        self.px = [[None] * N for _ in range(N)]

    def put(self, x, y, c):
        x, y = math.floor(x), math.floor(y)
        if 0 <= x < N and 0 <= y < N and c is not None:
            self.px[y][x] = c

    def rect(self, x, y, w, h, c):
        for j in range(h):
            for i in range(w):
                self.put(x + i, y + j, c)


def creature(f, cx, cy, r=9.0, glow=1.0, eyes="open", look=0, blush=False, squash=0.0, phase=0):
    """The eclipse: its corona's rays, its rim, its disc, its face."""
    ry = r * (1 - squash)
    rx = r * (1 + squash * 0.6)
    # Rays: short streamers round the rim, longer along the icon's tilt.
    if glow > 0:
        for k in range(8):
            t = k / 8 * 2 * math.pi + math.pi / 8
            bias = abs(math.cos(t - math.radians(-28))) ** 2
            # Every other ray a pixel longer, turn about, so it shimmers.
            length = (1 + (1 if (k + phase) % 2 else 0) + round(bias)) * glow
            for s in range(1, int(length) + 1):
                x = cx + (rx + 0.6 + s) * math.cos(t)
                y = cy + (ry + 0.6 + s) * math.sin(t)
                f.put(x, y, RAY if s == 1 else RAY_DIM)
    # Disc and rim.
    for y in range(N):
        for x in range(N):
            d = ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2
            if d <= 1.0:
                inner = ((x + 0.5 - cx) / (rx - 1.2)) ** 2 + ((y + 0.5 - cy) / (ry - 1.2)) ** 2
                if inner > 1.0:
                    hot = glow >= 0.8 and (x + 0.5 - cx) > 0 and (y + 0.5 - cy) < 0
                    f.put(x, y, (RIM_HOT if hot else RIM) if glow > 0.3 else RAY_DIM)
                else:
                    # Lit from the upper left, as the icon's disc.
                    lit = ((x - cx + 3) ** 2 + (y - cy + 3) ** 2) < (r * 0.45) ** 2
                    edge = inner > 0.78
                    f.put(x, y, DISC_EDGE if edge else (DISC_LIT if lit else DISC))
    # Face: eyes two pixels wide, three apart from the middle.
    ey = math.floor(cy)
    for side in (-1, 1):
        ex = math.floor(cx) + (side * 3) - (1 if side < 0 else 0) + look
        if eyes == "open":
            f.rect(ex, ey - 1, 2, 3, EYE)
        elif eyes == "down":
            f.rect(ex, ey + 1, 2, 2, EYE)
        elif eyes == "closed":
            f.rect(ex - 1 if side < 0 else ex, ey + 1, 3, 1, EYE)
        elif eyes == "happy":  # ^ ^
            f.put(ex - 1 if side < 0 else ex, ey + 1, EYE)
            f.rect(ex, ey, 1, 1, EYE) if side < 0 else f.rect(ex + 1, ey, 1, 1, EYE)
            f.put(ex + 1 if side < 0 else ex + 2, ey + 1, EYE)
        elif eyes == "wide":
            f.rect(ex, ey - 2, 2, 4, EYE)
            f.put(ex + (1 if side < 0 else 0), ey - 2, DISC_LIT)
        if blush:
            f.rect(ex - 1 if side < 0 else ex + 1, ey + 3, 2, 1, BLUSH)


def nub(f, x, y, c=RIM):
    """A little hand: two pixels of corona."""
    f.rect(x, y, 2, 2, c)


def keyboard(f, y, lit):
    f.rect(7, y, 18, 3, KEY)
    for i in range(8):
        f.put(8 + i * 2, y + 1, KEY_LIT if i in lit else SCREEN)


def check(f, x, y):
    f.rect(x, y, 6, 6, CHECK_BG)
    for (i, j) in [(1, 3), (2, 4), (3, 3), (4, 2), (4, 1)]:
        f.put(x + i, y + j, WHITE)


def bang(f, x, y):
    f.rect(x, y, 5, 7, BANG_BG)
    f.rect(x + 2, y + 1, 1, 3, BANG)
    f.put(x + 2, y + 5, BANG)


def zee(f, x, y, size):
    for i in range(size):
        f.put(x + i, y, ZZZ)
        f.put(x + i, y + size - 1, ZZZ)
        f.put(x + size - 1 - i, y + i, ZZZ)


def code(f, x, y, phase):
    # "</>" drifting up, one glyph at a time.
    glyphs = [
        [(0, 1), (1, 0), (1, 2)],  # <
        [(0, 2), (1, 1), (2, 0)],  # /
        [(0, 0), (1, 1), (0, 2)],  # >
    ]
    for g, pts in enumerate(glyphs):
        if g <= phase:
            for (i, j) in pts:
                f.put(x + g * 3 + i, y + j, CODE if g != 1 else CODE2)


def typing():
    frames = []
    for k in range(4):
        f = Frame()
        bob = 0 if k % 2 == 0 else 1
        creature(f, 16, 15 + bob, eyes="down", phase=k)
        keyboard(f, 25, {k, k + 3, (k * 5) % 8})
        # Hands on the keys, taking turns.
        nub(f, 9, 23 + (1 if k % 2 == 0 else 0), RIM_HOT)
        nub(f, 21, 23 + (0 if k % 2 == 0 else 1), RIM_HOT)
        code(f, 22, 2 + (3 - k) // 2, k % 3)
        frames.append(f)
    return frames


def happy():
    frames = []
    hops = [0, -3, -5, -3, 0, 0]
    for k, h in enumerate(hops):
        f = Frame()
        squash = 0.12 if h == 0 and k in (0, 5) else 0.0
        creature(f, 16, 18 + h, eyes="happy", blush=True, squash=squash, phase=k)
        if h < 0:
            nub(f, 4, 10 + h)
            nub(f, 26, 10 + h)
        check(f, 24, 1 + (1 if k % 2 else 0))
        # Sparks round it.
        for i, (sx, sy) in enumerate([(4, 4), (8, 1), (27, 12), (2, 14)]):
            if (k + i) % 3 == 0:
                f.put(sx, sy, SPARK)
                f.put(sx + 1, sy, SPARK)
                f.put(sx, sy + 1, SPARK)
        frames.append(f)
    return frames


def waiting():
    frames = []
    for k in range(4):
        f = Frame()
        creature(f, 15, 18, eyes="wide", look=1, phase=k)
        # A hand up, waving a little.
        wave = [0, -1, 0, 1][k]
        nub(f, 26 + wave, 6, RIM_HOT)
        # The arm, from its side up to the hand.
        for (ax, ay) in [(24, 13), (25, 12), (25, 11), (26, 10), (26, 9), (26 + (wave > 0), 8)]:
            f.put(ax, ay, RIM)
        if k != 3:
            bang(f, 4, 1)
        frames.append(f)
    return frames


def sleeping():
    frames = []
    for k in range(4):
        f = Frame()
        breathe = 0.05 if k % 2 else 0.0
        creature(f, 15, 19, eyes="closed", glow=0.25, squash=breathe)
        zee(f, 22, 9 - k, 4)
        if k >= 2:
            zee(f, 26, 3 - (k - 2), 5)
        frames.append(f)
    return frames


def png(path, frames):
    w, h = N * SCALE * len(frames), N * SCALE
    rows = []
    for y in range(h):
        row = bytearray([0])
        for x in range(w):
            f = frames[x // (N * SCALE)]
            c = f.px[y // SCALE][(x % (N * SCALE)) // SCALE]
            row += bytes(c + (255,)) if c else b"\0\0\0\0"
        rows.append(bytes(row))
    raw = b"".join(rows)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    with open(path, "wb") as out:
        out.write(b"\x89PNG\r\n\x1a\n")
        out.write(chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)))
        out.write(chunk(b"IDAT", zlib.compress(raw, 9)))
        out.write(chunk(b"IEND", b""))


if __name__ == "__main__":
    for name, make in [("typing", typing), ("happy", happy), ("waiting", waiting), ("sleeping", sleeping)]:
        frames = make()
        png(os.path.join(HERE, f"{name}.png"), frames)
        print(name, len(frames))

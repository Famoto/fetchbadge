"""Shaded crescent for config.rs LOGO (patch-style shape, opening to the left).
Inner edge light, outer rim heavy; edges supersampled so the tips taper. Paste the output into LOGO.
Cells are 1:2 (10x20 px), so a 38x18 grid draws a round disk."""
import math, pathlib
COLS, ROWS = 38, 18
INNER = (-0.42, 0.0, 0.92)   # inner circle (cx, cy, r); negative cx = opens to the left
blob = (pathlib.Path(__file__).resolve().parent.parent / "firmware/src/fonts/fira_10x20.bin").read_bytes()
def ink(ch):
    g = blob[(ord(ch) - 0x20) * 100:(ord(ch) - 0x1F) * 100]
    return sum((b >> 4) + (b & 15) for b in g)
RAMP = "".join(sorted(" .,:;-~=+*ocxa%e#&8@", key=ink))
SPARKLES = [(9, 4, 2), (5, 10, 1), (13, 13, 2), (16, 8, 1), (2, 2, 1)]
g = [[" "] * COLS for _ in range(ROWS)]
N = 4  # supersampling per cell axis: edge cells get lighter glyphs, so the tips taper
for r in range(ROWS):
    for c in range(COLS):
        acc = 0.0
        for sy in range(N):
            for sx in range(N):
                x = (c + (sx + 0.5) / N) / COLS * 2 - 1
                y = (r + (sy + 0.5) / N) / ROWS * 2 - 1
                ro = math.hypot(x, y)
                ri = math.hypot(x - INNER[0], y - INNER[1]) / INNER[2]
                if ro > 1 or ri < 1:
                    continue
                # 0 at the inner (lit) edge, 1 at the outer rim
                t = (ri - 1) / max(1e-6, (ri - 1) + (1 - ro) / INNER[2])
                acc += 0.3 + 0.7 * t ** 0.8
        v = acc / (N * N)
        if v > 0.06:
            g[r][c] = RAMP[min(len(RAMP) - 1, 1 + int(v * (len(RAMP) - 2)))]
def put(c, r, ch):
    if 0 <= r < ROWS and 0 <= c < COLS and g[r][c] == " ": g[r][c] = ch
for c, r, s in SPARKLES:
    if s == 1: put(c, r, "+")
    else: put(c, r-1, "|"); put(c-1, r, "-"); put(c, r, "+"); put(c+1, r, "-"); put(c, r+1, "|")
print("\n".join("".join(r).rstrip() for r in g))

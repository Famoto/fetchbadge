"""Rasterise Fira Code (ASCII 0x20-0x7E) into 4-bit antialiased cell fonts for badge/font.rs.
Run: python3 fontgen.py /path/to/FiraCode-Medium.ttf   (needs Pillow; OFL-licensed font)
Writes ../firmware/src/fonts/fira_<w>x<h>.bin: per glyph, h rows of w/2 bytes, high nibble first, 15 = full ink."""
import sys, pathlib
from PIL import Image, ImageDraw, ImageFont

out = pathlib.Path(__file__).resolve().parent.parent / "firmware/src/fonts"
out.mkdir(parents=True, exist_ok=True)
ttf = sys.argv[1]

for w, h in [(10, 20), (16, 32), (32, 64)]:
    size = w / ImageFont.truetype(ttf, 100).getlength("M") * 100  # monospace advance == w
    font = ImageFont.truetype(ttf, round(size, 1))
    asc, desc = font.getmetrics()
    base = (h - (asc + desc)) // 2 + asc
    data = bytearray()
    for c in range(0x20, 0x7F):
        img = Image.new("L", (w, h), 0)
        ImageDraw.Draw(img).text((0, base), chr(c), font=font, fill=255, anchor="ls")
        px = [v * 15 // 255 for v in img.tobytes()]
        data += bytes((px[i] << 4) | px[i + 1] for i in range(0, len(px), 2))
    (out / f"fira_{w}x{h}.bin").write_bytes(data)
    print(f"fira_{w}x{h}.bin  size={size:.1f}px  {len(data)} bytes")

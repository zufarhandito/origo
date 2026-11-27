"""Regenerate embedded Windows/title-bar icons from the supplied branding PNG.
Run: python scripts/update-origo-icon.py (requires Pillow).
"""
from pathlib import Path
import io
import struct
from PIL import Image

root = Path(__file__).resolve().parents[1] / "apps/desktop/resources/branding"
source = Image.open(root / "origo-source.png").convert("RGBA")
side = max(source.size)
canvas = Image.new("RGBA", (side, side), source.getpixel((0, 0)))
canvas.alpha_composite(source, ((side-source.width)//2, (side-source.height)//2))
sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
blobs = []
for size in sizes:
    resized = canvas.resize((size, size), Image.Resampling.LANCZOS)
    px = resized.load()
    for y in range(size):
        for x in range(size):
            if px[x, y][3] == 0:
                px[x, y] = (0, 0, 0, 0)
    stream = io.BytesIO()
    resized.save(stream, format="PNG")
    blobs.append(stream.getvalue())
    if size == 32:
        (root / "origo-32.png").write_bytes(stream.getvalue())
offset = 6 + 16 * len(sizes)
entries = []
for size, blob in zip(sizes, blobs):
    entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(blob), offset))
    offset += len(blob)
(root / "origo.ico").write_bytes(struct.pack("<HHH", 0, 1, len(sizes)) + b"".join(entries) + b"".join(blobs))
with Image.open(root / "origo.ico") as icon:
    assert icon.ico.sizes() == {(size, size) for size in sizes}
print("Updated origo.ico (9 sizes) and origo-32.png")

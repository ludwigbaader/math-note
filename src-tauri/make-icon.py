"""Generates app-icon.png (1024x1024) without any third-party dependencies.
Run `npx tauri icon src-tauri/app-icon.png` afterwards to produce the platform icons."""
import struct, zlib

SIZE = 1024

def inside_rounded(x, y, x0, y0, x1, y1, r):
    if x < x0 or x >= x1 or y < y0 or y >= y1:
        return False
    cx = min(max(x, x0 + r), x1 - r - 1)
    cy = min(max(y, y0 + r), y1 - r - 1)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r

rows = []
for y in range(SIZE):
    row = bytearray([0])  # filter type: none
    t = y / SIZE
    bg = (int(58 + 30 * t), int(96 + 40 * t), int(214 - 10 * t))
    for x in range(SIZE):
        if not inside_rounded(x, y, 0, 0, SIZE, SIZE, 224):
            row += b"\x00\x00\x00\x00"
        elif inside_rounded(x, y, 232, 386, 792, 470, 42) or inside_rounded(x, y, 232, 554, 792, 638, 42):
            row += b"\xff\xff\xff\xff"
        else:
            row += bytes(bg) + b"\xff"
    rows.append(bytes(row))

def chunk(tag, data):
    body = tag + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
png += chunk(b"IEND", b"")
with open("src-tauri/app-icon.png", "wb") as f:
    f.write(png)
print("wrote src-tauri/app-icon.png")

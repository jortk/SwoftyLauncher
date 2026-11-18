#!/usr/bin/env python3
import math
import struct
import sys
import zlib
from pathlib import Path

PX = 3


def png(path, w, h, pixel):
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        for x in range(w):
            raw += bytes(pixel(x, y))
    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))
    out = b"\x89PNG\r\n\x1a\n"
    out += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    out += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    out += chunk(b"IEND", b"")
    Path(path).write_bytes(out)


def rounded(w, h, r, colour):
    def pixel(x, y):
        dx = max(r - x - 0.5, x + 0.5 - (w - r), 0)
        dy = max(r - y - 0.5, y + 0.5 - (h - r), 0)
        return colour if math.hypot(dx, dy) <= r else (0, 0, 0, 0)
    return pixel


def ring(size, inner, outer, colour):
    c = size / 2
    def pixel(x, y):
        d = math.hypot(x + 0.5 - c, y + 0.5 - c)
        return colour if inner <= d <= outer else (0, 0, 0, 0)
    return pixel


def main(folder):
    folder = Path(folder)
    folder.mkdir(parents=True, exist_ok=True)
    png(folder / "base.png", 120 * PX, 72 * PX, rounded(120 * PX, 72 * PX, 12 * PX, (16, 18, 24, 200)))
    png(folder / "ring.png", 48 * PX, 48 * PX, ring(48 * PX, 19 * PX, 24 * PX, (255, 255, 255, 255)))
    png(folder / "solid.png", 8 * PX, 8 * PX, lambda x, y: (255, 255, 255, 255))
    png(folder / "fill.png", 80 * PX, 8 * PX, rounded(80 * PX, 8 * PX, 4 * PX, (255, 214, 10, 255)))
    png(folder / "lamp.png", 16 * PX, 8 * PX, rounded(16 * PX, 8 * PX, 4 * PX, (255, 80, 40, 255)))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "art")

#!/usr/bin/env python3
"""Writes the tower's sprites: flat fills the composer tints or draws as they are."""
import struct
import sys
import zlib
from pathlib import Path


def png(path, w, h, colour):
    raw = bytearray()
    for _ in range(h):
        raw.append(0)
        raw += bytes(colour) * w

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    out = b"\x89PNG\r\n\x1a\n"
    out += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    out += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    out += chunk(b"IEND", b"")
    Path(path).write_bytes(out)


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "art")
    out.mkdir(parents=True, exist_ok=True)
    png(out / "hdr.png", 64, 16, (12, 16, 28, 230))
    png(out / "strip.png", 64, 16, (20, 24, 36, 215))
    png(out / "me.png", 64, 16, (255, 255, 255, 40))
    png(out / "bar.png", 8, 16, (255, 255, 255, 255))
    png(out / "card.png", 64, 16, (255, 255, 255, 255))
    png(out / "sep.png", 64, 4, (255, 255, 255, 60))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Packs the core's SWOFTY_POSTFX_CLIP capture into a shader preview clip zip.

    make_preview_clip.py <ppm dir> <out.zip> --title "Cockpit lap" --fps 30 --view cockpit

Reads clip-<frame>.ppm (1280x720), writes 0000.webp (lossy, quality 88) and clip.json, zips them and
prints the zip's size and sha256 for the clips.json index.
"""

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path


def ppm_size(path):
    with open(path, "rb") as f:
        header = f.read(64)
    m = re.match(rb"P6\s+(?:#[^\n]*\n\s*)*(\d+)\s+(\d+)\s+(\d+)\s", header)
    if not m:
        raise SystemExit(f"{path}: not a binary PPM")
    return int(m.group(1)), int(m.group(2))


def encoder():
    try:
        from PIL import Image

        def pillow(src, dst, quality):
            with Image.open(src) as im:
                im.convert("RGB").save(dst, "WEBP", quality=quality, method=4)

        return pillow
    except ImportError:
        pass
    if shutil.which("cwebp"):
        return lambda src, dst, q: subprocess.run(
            ["cwebp", "-quiet", "-q", str(q), str(src), "-o", str(dst)], check=True
        )
    magick = shutil.which("magick") or shutil.which("convert")
    if magick:
        return lambda src, dst, q: subprocess.run(
            [magick, str(src), "-quality", str(q), str(dst)], check=True
        )
    raise SystemExit("needs Pillow, cwebp or ImageMagick")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ppm_dir", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--title", required=True)
    ap.add_argument("--fps", type=float, default=30)
    ap.add_argument("--view", default="cockpit")
    ap.add_argument("--quality", type=int, default=88)
    args = ap.parse_args()

    frames = sorted(
        args.ppm_dir.glob("clip-*.ppm"),
        key=lambda p: int(re.search(r"clip-(\d+)", p.name).group(1)),
    )
    if not frames:
        raise SystemExit(f"no clip-<frame>.ppm in {args.ppm_dir}")
    width, height = ppm_size(frames[0])
    encode = encoder()

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        for i, src in enumerate(frames):
            encode(src, tmp / f"{i:04d}.webp", args.quality)
        (tmp / "clip.json").write_text(
            json.dumps(
                {
                    "title": args.title,
                    "fps": args.fps,
                    "width": width,
                    "height": height,
                    "frames": len(frames),
                    "view": args.view,
                },
                indent=2,
            )
            + "\n"
        )
        args.out.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(args.out, "w", zipfile.ZIP_STORED) as z:
            z.write(tmp / "clip.json", "clip.json")
            for i in range(len(frames)):
                z.write(tmp / f"{i:04d}.webp", f"{i:04d}.webp")

    data = args.out.read_bytes()
    print(f"{args.out}: {len(frames)} frames, {width}x{height}")
    print(f"size   {len(data)}")
    print(f"sha256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    sys.exit(main())

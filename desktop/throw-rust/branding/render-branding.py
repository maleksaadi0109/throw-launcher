#!/usr/bin/env python3
"""Render the supplied SVG; build-time tooling only, not a runtime dependency.

Requires ImageMagick with an SVG delegate. Run from any working directory:
    python desktop/throw-rust/branding/render-branding.py

The supplied vector remains untouched. All ICO frames are independently
Lanczos-filtered from a 2048px raster, including the smallest Windows sizes.
PNG-compressed ICO frames retain full alpha and are supported by Windows Vista+.
"""

import shutil
import struct
import subprocess
import tempfile
from pathlib import Path


SIZES = (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)
DESKTOP = Path(__file__).resolve().parents[1]
ROOT = DESKTOP.parents[1]
SOURCE = ROOT / "branding/throw-launcher.svg"


def main():
    renderer = shutil.which("magick") or shutil.which("convert")
    if not renderer:
        raise SystemExit("Install ImageMagick with SVG support to render branding.")
    if not SOURCE.is_file():
        raise SystemExit(f"Missing supplied artwork: {SOURCE}")

    def render(*args):
        subprocess.run([renderer, *map(str, args)], check=True)

    with tempfile.TemporaryDirectory(prefix="throw-branding-") as directory:
        temp = Path(directory)
        master = temp / "master.png"
        # 32 SVG units at 6144 DPI / 96 CSS DPI = 2048 raster pixels.
        render(
            "-background", "none", "-density", "6144", SOURCE,
            "-resize", "2048x2048!", "-strip", f"PNG32:{master}",
        )
        png = temp / "throw-launcher.png"
        render(
            master, "-filter", "Lanczos", "-resize", "1024x1024!",
            "-strip", f"PNG32:{png}",
        )
        frames = []
        for size in SIZES:
            frame = temp / f"{size}.png"
            render(
                master, "-filter", "Lanczos", "-resize", f"{size}x{size}!",
                "-strip", f"PNG32:{frame}",
            )
            frames.append(frame.read_bytes())

        offset = 6 + 16 * len(SIZES)
        entries = []
        for size, frame in zip(SIZES, frames):
            dimension = size if size < 256 else 0
            entries.append(struct.pack(
                "<BBBBHHII", dimension, dimension, 0, 0,
                1, 32, len(frame), offset,
            ))
            offset += len(frame)
        ico = struct.pack("<HHH", 0, 1, len(SIZES)) + b"".join(entries + frames)
        # Render everything before replacing any output.
        (ROOT / "branding/throw-launcher.png").write_bytes(png.read_bytes())
        (ROOT / "branding/throw-launcher.ico").write_bytes(ico)
        (DESKTOP / "branding/throw-rust.ico").write_bytes(ico)
        print("Rendered 1024px PNG and matching ICOs:", ", ".join(map(str, SIZES)))


if __name__ == "__main__":
    main()

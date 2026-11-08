#!/usr/bin/env python3
"""Draw the original toolbar PNG masks with only the Python standard library.

24-unit geometry is rasterized at 4x with antialiased, round-ended strokes.
These app-owned drawings contain no font glyphs or third-party icon artwork.
"""
import math
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1] / "src/assets/toolbar"
SIZE = 96


def path(*points):
    return list(zip(points, points[1:]))


def arc(x, y, radius, start, end):
    return path(*[
        (x + radius * math.cos(math.radians(a)),
         y + radius * math.sin(math.radians(a)))
        for a in range(start, end + 1, 5)
    ])


def star(x, y, radius):
    short = radius * 0.3
    return path((x, y-radius), (x+short, y-short), (x+radius, y),
                (x+short, y+short), (x, y+radius), (x-short, y+short),
                (x-radius, y), (x-short, y-short), (x, y-radius))


ICONS = {
    "create-empty": path((13, 3), (5, 3), (5, 21), (15, 21))
        + path((13, 3), (18, 8), (13, 8), (13, 3))
        + path((19, 13), (19, 21)) + path((15, 17), (23, 17)),
    "export-icloud": arc(8, 13, 4, 90, 270) + arc(13, 9, 5, 185, 355)
        + arc(18, 13, 4, 270, 450) + path((8, 17), (9, 17))
        + path((15, 17), (18, 17)) + path((12, 22), (12, 12))
        + path((9, 15), (12, 12), (15, 15)),
    "export-notion": path((4, 3), (17, 3), (20, 6), (20, 21), (4, 21), (4, 3))
        + path((8, 17), (8, 8), (16, 17), (16, 8))
        + path((7, 8), (9, 8)) + path((15, 8), (17, 8)),
    "command-palette": path((4, 5), (20, 5), (20, 19), (4, 19), (4, 5))
        + path((8, 9), (11, 12), (8, 15)) + path((13, 15), (16, 15)),
    "trash-recovery": path((4, 7), (16, 7)) + path((8, 4), (12, 4))
        + path((5, 7), (6, 20), (12, 20)) + path((15, 7), (14.7, 11))
        + path((9, 10), (9, 16)) + arc(17, 17, 4, 0, 285)
        + path((18, 11), (18, 14), (21, 14)),
    "ai-activities": path((3, 4), (3, 20), (21, 20))
        + path((6, 13), (9, 13), (11, 7), (15, 17), (17, 11), (21, 11))
        + star(19, 4, 2),
    "save": path((4, 3), (17, 3), (21, 7), (21, 21), (3, 21), (3, 3), (4, 3))
        + path((7, 3), (7, 9), (16, 9), (16, 3))
        + path((7, 21), (7, 14), (17, 14), (17, 21)),
    "generate": star(10, 14, 7) + star(19, 5, 3) + star(20, 19, 2),
}


def distance(x, y, a, b):
    dx, dy = b[0]-a[0], b[1]-a[1]
    t = max(0, min(1, ((x-a[0])*dx + (y-a[1])*dy) / (dx*dx+dy*dy)))
    return math.hypot(x-a[0]-t*dx, y-a[1]-t*dy)


def chunk(kind, data):
    return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind+data))


def draw(segments):
    rows = bytearray()
    for row in range(SIZE):
        rows.append(0)
        for col in range(SIZE):
            d = min(distance((col+.5)/4, (row+.5)/4, a, b) for a, b in segments)
            alpha = round(255 * max(0, min(1, (.8-d)*4+.5)))
            rows.extend((255, 255, 255, alpha))
    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('!2I5B', SIZE, SIZE, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(rows, 9)) + chunk(b'IEND', b''))


if __name__ == '__main__':
    ROOT.mkdir(parents=True, exist_ok=True)
    for name, segments in ICONS.items():
        (ROOT / f'{name}.png').write_bytes(draw(segments))

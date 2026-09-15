"""Export our rectangle-based vector logo to native icon formats; no third-party tools."""
from pathlib import Path
import struct
import xml.etree.ElementTree as ET
import zlib

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"


def pixels(width, height):
    shapes = ET.parse(ASSETS / "logo.svg").getroot().findall("{http://www.w3.org/2000/svg}rect")
    result = bytearray(width * height * 4)
    for rect in shapes:
        x, y, w, h = (int(rect.get(k)) for k in ("x", "y", "width", "height"))
        color = bytes.fromhex(rect.get("fill")[1:]) + b"\xff"
        for row in range(y * height // 256, (y + h) * height // 256):
            for col in range(x * width // 256, (x + w) * width // 256):
                pos = (row * width + col) * 4
                result[pos:pos + 4] = color
    return result


def png(width, height):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    data = pixels(width, height)
    rows = b"".join(b"\0" + data[y * width * 4:(y + 1) * width * 4] for y in range(height))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def bmp(width, height):
    # Installer banners put the square logo at the right, against the brand background.
    size = min(width, height)
    logo = pixels(size, size)
    stride = (width * 3 + 3) & ~3
    rows = bytearray()
    for y in reversed(range(height)):
        row = bytearray()
        for x in range(width):
            if x >= width - size:
                p = (y * size + x - (width - size)) * 4
                row.extend(logo[p:p + 3][::-1])
            else:
                row.extend(bytes.fromhex("26170d"))
        rows.extend(row + bytes(stride - width * 3))
    return (b"BM" + struct.pack("<IHHI", 54 + len(rows), 0, 0, 54)
            + struct.pack("<IiiHHIIiiII", 40, width, height, 1, 24, 0, len(rows), 2835, 2835, 0, 0) + rows)


def main():
    image = png(256, 256)
    (ASSETS / "logo.png").write_bytes(image)
    (ASSETS / "logo.ico").write_bytes(struct.pack("<HHH", 0, 1, 1)
        + struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(image), 22) + image)
    chunks = b"".join(kind + struct.pack(">I", len(data) + 8) + data
                      for kind, data in [(b"ic08", image), (b"ic09", png(512, 512)), (b"ic10", png(1024, 1024))])
    (ASSETS / "logo.icns").write_bytes(b"icns" + struct.pack(">I", len(chunks) + 8) + chunks)
    (ASSETS / "banner.bmp").write_bytes(bmp(493, 58))
    (ASSETS / "dialog.bmp").write_bytes(bmp(493, 312))


if __name__ == "__main__":
    main()

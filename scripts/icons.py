"""Package SVG-derived build icons with the same renderer used by the app and tray."""
from pathlib import Path
import subprocess
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1]

def png(size):
    rgba = rasterized[size]
    pixels = b''.join(b'\0' + rgba[y*size*4:(y+1)*size*4] for y in range(size))
    def chunk(kind, value):
        return struct.pack('>I', len(value)) + kind + value + struct.pack('>I', zlib.crc32(kind + value))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>2I5B', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(bytes(pixels), 9)) + chunk(b'IEND', b'')

if __name__ == '__main__':
    sizes_to_render = [16, 32, 48, 64, 128, 256, 512, 1024]
    raw = subprocess.check_output(
        ['cargo', 'run', '--locked', '--quiet', '--example', 'render_icons', '--',
         *map(str, sizes_to_render)], cwd=ROOT)
    rasterized = {}
    offset = 0
    for size in sizes_to_render:
        length = size * size * 4
        rasterized[size] = raw[offset:offset + length]
        offset += length
    assert len(raw) == offset, 'Unexpected icon renderer output'
    assets = ROOT / 'assets'
    assets.mkdir(exist_ok=True)
    (assets / 'icon.png').write_bytes(png(256))
    sizes = [16, 32, 48, 64, 128, 256]
    entries = [png(n) for n in sizes]
    offset = 6 + 16 * len(entries)
    headers = bytearray(struct.pack('<3H', 0, 1, len(entries)))
    for size, data in zip(sizes, entries):
        headers.extend(struct.pack('<4B2H2I', size % 256, size % 256, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    (assets / 'icon.ico').write_bytes(headers + b''.join(entries))
    chunks = []
    # Finder can corrupt embedded 16pt/32pt representations when applying
    # macOS icon styling. Let Icon Services scale down the 128pt vector render
    # for those views. Keep native 1x/2x slots for the larger point sizes.
    mac_slots = [
        (b'ic07', 128), (b'ic13', 256),
        (b'ic08', 256), (b'ic14', 512),
        (b'ic09', 512), (b'ic10', 1024),
    ]
    for tag, size in mac_slots:
        data = png(size)
        chunks.append(tag + struct.pack('>I', len(data)+8) + data)
    data = b''.join(chunks)
    (assets / 'icon.icns').write_bytes(b'icns' + struct.pack('>I', len(data)+8) + data)

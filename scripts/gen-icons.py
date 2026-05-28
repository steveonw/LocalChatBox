"""Generate Tauri app icons for LocalChatBox using only Python stdlib."""
import os, struct, zlib

OUT = os.path.join(os.path.dirname(__file__), "../src-tauri/icons")
os.makedirs(OUT, exist_ok=True)

# Colour palette
BG   = (30,  33,  40)   # dark panel
ACC  = (99, 179, 237)   # blue accent
FG   = (244, 239, 230)  # off-white

def make_png(size: int) -> bytes:
    """Create a simple LocalChatBox icon as a PNG byte string."""
    w = h = size
    # Build RGBA pixel grid
    pixels = []
    for y in range(h):
        row = []
        for x in range(w):
            cx, cy = x - w/2, y - h/2
            r = (cx**2 + cy**2) ** 0.5
            # Outer rounded-rect border
            mx, my = abs(cx) / (w * 0.44), abs(cy) / (h * 0.44)
            m = (mx**8 + my**8) ** 0.125
            if m > 1.0:
                row.append((0, 0, 0, 0))  # transparent
                continue
            # Chat-bubble body
            bx, by = abs(cx) / (w * 0.38), abs(cy) / (h * 0.34)
            bm = (bx**6 + by**6) ** (1/6)
            # Tail: bottom-left bump
            tail = (cx + w*0.28)**2 + (cy - h*0.30)**2 < (w*0.10)**2
            if bm < 1.0 or tail:
                # Three dots inside bubble
                dot_y = cy / (h * 0.08)
                dot_positions = [-0.30, 0.0, 0.30]
                on_dot = any(
                    ((cx / w - dp)**2 + dot_y**2) < (0.055)**2
                    for dp in dot_positions
                )
                if on_dot:
                    row.append((*BG, 255))
                else:
                    # Slight gradient: lighter at top
                    blend = max(0.0, min(1.0, (cy / h + 0.5) * 0.3))
                    c = tuple(int(ACC[i] + (FG[i] - ACC[i]) * blend) for i in range(3))
                    row.append((*c, 255))
            else:
                row.append((*BG, 220))
        pixels.append(row)

    # Encode as PNG (RGBA, 8-bit)
    def chunk(tag: bytes, data: bytes) -> bytes:
        c = tag + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    raw = b""
    for row in pixels:
        raw += b"\x00"
        for px in row:
            raw += bytes(px)

    sig   = b"\x89PNG\r\n\x1a\n"
    ihdr  = chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    idat  = chunk(b"IDAT", zlib.compress(raw, 9))
    iend  = chunk(b"IEND", b"")
    return sig + ihdr + idat + iend

def make_ico(pngs: list[tuple[int, bytes]]) -> bytes:
    """Wrap PNG blobs in an ICO container (modern PNG-in-ICO format)."""
    n = len(pngs)
    header = struct.pack("<HHH", 0, 1, n)
    offset = 6 + n * 16
    entries = b""
    data = b""
    for size, blob in pngs:
        s = min(size, 255)  # ICO field is 1 byte; 0 means 256
        entries += struct.pack("<BBBBHHII", s, s, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
        data += blob
    return header + entries + data

print("Generating icons …")

sizes = {
    "32x32.png":       32,
    "128x128.png":     128,
    "128x128@2x.png":  256,
    "icon.png":        512,
}

blobs: dict[int, bytes] = {}
for name, sz in sizes.items():
    blob = make_png(sz)
    path = os.path.join(OUT, name)
    with open(path, "wb") as f:
        f.write(blob)
    blobs[sz] = blob
    print(f"  wrote {name} ({sz}×{sz})")

# ICO: embed 16, 32, 48, 256 – generate smaller ones on the fly
ico_sizes = [16, 32, 48, 256]
ico_blobs = [(sz, blobs.get(sz) or make_png(sz)) for sz in ico_sizes]
ico_path = os.path.join(OUT, "icon.ico")
with open(ico_path, "wb") as f:
    f.write(make_ico(ico_blobs))
print(f"  wrote icon.ico  ({', '.join(str(s) for s,_ in ico_blobs)})")

# macOS ICNS (minimal: just wrap the 512px PNG as ic09)
icns_path = os.path.join(OUT, "icon.icns")
blob512 = blobs[512]
icns_inner = b"ic09" + struct.pack(">I", 8 + len(blob512)) + blob512
icns_data  = b"icns" + struct.pack(">I", 8 + len(icns_inner)) + icns_inner
with open(icns_path, "wb") as f:
    f.write(icns_data)
print(f"  wrote icon.icns")

print("Done.")

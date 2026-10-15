import numpy as np, json, math
from PIL import Image, ImageDraw, ImageFont
from collections import Counter
cells = {tuple(map(int, k.split(","))): v for k, v in json.load(open("cells.json")).items()}
rgb = Image.open("real_board.png").convert("RGB"); A = np.asarray(rgb).astype(int); H, W, _ = A.shape
L = (0.3 * A[..., 0] + 0.59 * A[..., 1] + 0.11 * A[..., 2])
yy, xx = np.mgrid[-34:35, -34:35]
disk = (xx**2 + yy**2) <= 30**2
inner = (xx**2 + yy**2) <= 22**2
ring = ((xx**2 + yy**2) > 26**2) & ((xx**2 + yy**2) <= 34**2)
def patch(arr, cx, cy):
    x0, y0 = int(round(cx)) - 34, int(round(cy)) - 34
    if x0 < 0 or y0 < 0 or x0 + 69 > W or y0 + 69 > H: return None
    return arr[y0:y0+69, x0:x0+69]
for (q, r), c in cells.items():
    P = patch(A, c["cx"], c["cy"]); PL = patch(L, c["cx"], c["cy"])
    if P is None: c["kind"] = "sea"; continue
    px = P[disk]; med = np.median(px, axis=0); R, G, B = med; lum = np.median(PL[disk])
    if B > R + 25 and B > 120: kind = "sea"
    elif G > R + 8 and G > B + 20: kind = "green"
    elif lum < 95: kind = "rock"
    elif R - B > 35 and lum > 178: kind = "pale"
    elif R - B > 35: kind = "brown"
    else: kind = "other"
    c["kind"] = kind; c["lum"] = float(lum); c["med"] = [int(R), int(G), int(B)]
    d, g = P[inner], P[ring]
    c["contrast"] = float(np.abs(d - g.mean(axis=0)).sum(axis=1).mean())
def zone_of(c):
    k, x, y = c["kind"], c["cx"], c["cy"]
    if k in ("sea", "other"): return "~"
    if k == "pale": return "B" if x < W / 2 else "D"
    if k == "green": return "F" if y < H / 2 else "G"
    if k == "brown":
        if x < 640 and y < 560: return "A"
        if x > 980 and y > 1080: return "E"
        return "C"
    return None
for c in cells.values(): c["zone"] = zone_of(c)
def neigh(q, r): return [(q+1,r),(q-1,r),(q,r+1),(q,r-1),(q+1,r-1),(q-1,r+1)]
for _ in range(3):
    for (q, r), c in cells.items():
        if c["zone"] is None:
            zs = [cells[n]["zone"] for n in neigh(q, r) if n in cells and cells[n]["zone"] not in (None, "~")]
            if zs: c["zone"] = Counter(zs).most_common(1)[0][0]
for c in cells.values():
    if c["zone"] is None: c["zone"] = "~"
print(Counter(c["zone"] for c in cells.values()))
print(Counter((c["zone"], c["kind"]) for c in cells.values() if c["zone"] != "~"))
json.dump({f"{q},{r}": c for (q, r), c in cells.items()}, open("cells2.json", "w"), default=float)

PITCH = 81.8; ANG = math.radians(-15); Rr = PITCH / math.sqrt(3)
def verts(cx, cy): return [(cx + Rr * math.cos(ANG + math.radians(30 + 60*k)), cy + Rr * math.sin(ANG + math.radians(30 + 60*k))) for k in range(6)]
COL = {"~": (60, 120, 220), "A": (230, 60, 60), "B": (255, 230, 90), "C": (220, 120, 30), "D": (255, 230, 90), "E": (60, 90, 230), "F": (40, 200, 40), "G": (40, 200, 40)}
try: font = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 15)
except Exception: font = ImageFont.load_default()
ov = rgb.copy(); d = ImageDraw.Draw(ov)
for (q, r), c in cells.items():
    z = c["zone"]; d.polygon(verts(c["cx"], c["cy"]), outline=COL[z], width=2)
    if z != "~" and (c["kind"] == "rock" or c.get("contrast", 0) > 55):
        t = ("#" if c["kind"] == "rock" else "") + f"{q},{r}"
        d.text((c["cx"] - 18, c["cy"] - 8), t, fill=(0, 0, 0), font=font)
        d.text((c["cx"] - 19, c["cy"] - 9), t, fill=(255, 255, 255), font=font)
boxes = {"A": (40, 200, 600, 640), "B": (60, 480, 660, 1100), "C": (420, 320, 1100, 1220), "D": (960, 480, 1560, 1100), "E": (960, 1040, 1560, 1450)}
for z, b in boxes.items():
    crop = ov.crop(b); crop.resize((crop.width * 3 // 2, crop.height * 3 // 2)).save(f"zone_{z}.png")

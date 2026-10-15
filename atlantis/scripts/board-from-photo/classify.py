import numpy as np, json, math
from PIL import Image, ImageDraw, ImageFilter
from collections import Counter

rgb = Image.open("real_board.png").convert("RGB")
A = np.asarray(rgb).astype(int)
H, W, _ = A.shape
gray = np.asarray(rgb.convert("L")).astype(float)
hpf = gray - np.asarray(rgb.convert("L").filter(ImageFilter.GaussianBlur(6))).astype(float)

PITCH = 81.8
ANG = math.radians(-15)
v1 = np.array([math.cos(ANG), math.sin(ANG)]) * PITCH             # q axis
v2 = np.array([math.cos(ANG + math.pi/3), math.sin(ANG + math.pi/3)]) * PITCH  # r axis
Rr = PITCH / math.sqrt(3)
def center(q, r, o): return o + q * v1 + r * v2
def verts(cx, cy):
    return [(cx + Rr * math.cos(ANG + math.radians(30 + 60*k)), cy + Rr * math.sin(ANG + math.radians(30 + 60*k))) for k in range(6)]

# Phase: bright vertices over three sand regions.
def phase_score(o):
    s = 0.0; n = 0
    for q in range(-30, 30):
        for r in range(-30, 30):
            cx, cy = center(q, r, o)
            if not ((110 < cx < 470 and 560 < cy < 1000) or (1130 < cx < 1480 and 560 < cy < 1000) or (620 < cx < 1000 and 500 < cy < 900)):
                continue
            for vx, vy in verts(cx, cy):
                xi, yi = int(round(vx)), int(round(vy))
                if 0 <= xi < W and 0 <= yi < H:
                    s += hpf[yi, xi]; n += 1
    return s / max(n, 1)
best = None
for a in np.arange(0, 1, 0.04):
    for b in np.arange(0, 1, 0.04):
        o = a * v1 + b * v2
        sc = phase_score(o)
        if best is None or sc > best[0]: best = (sc, o)
sc, O = best
print("phase score %.2f origin %s" % (sc, O.round(1)))

# Disk masks.
yy, xx = np.mgrid[-30:31, -30:31]
d_small = (xx**2 + yy**2) <= 12**2
d_icon = (xx**2 + yy**2) <= 26**2
def sample(cx, cy, mask):
    x0, y0 = int(round(cx)) - 30, int(round(cy)) - 30
    if x0 < 0 or y0 < 0 or x0 + 61 > W or y0 + 61 > H: return None
    return A[y0:y0+61, x0:x0+61][mask]

cells = {}
for q in range(-40, 40):
    for r in range(-40, 40):
        cx, cy = center(q, r, O)
        if not (Rr < cx < W - Rr and Rr < cy < H - Rr): continue
        px = sample(cx, cy, d_small)
        if px is None: continue
        R, G, B = px.mean(axis=0)
        icon = sample(cx, cy, d_icon)
        iR, iG, iB = icon[:, 0], icon[:, 1], icon[:, 2]
        orange = ((iR > 150) & (iR - iB > 90) & (iG > 70) & (iG < 170) & (iR - iG > 40)).mean()
        yellow = ((iR > 200) & (iG > 150) & (iB < 90)).mean()
        star = ((iB > 170) & (iB - iR > 40) & (iG > 150) & (iR > 90)).mean()
        greyi = ((abs(iR - iG) < 25) & (abs(iG - iB) < 30) & (iR > 95) & (iR < 175)).mean()
        if B > R + 25 and B > 120: kind = "sea"
        elif G > R + 8 and G > B + 20: kind = "green"
        elif max(R, G, B) < 95: kind = "rock"
        elif R > 195 and G > 160 and B > 105: kind = "pale"
        elif R > 120 and G > 80 and B < 130 and R - B > 40: kind = "brown"
        else: kind = "other"
        cells[(q, r)] = dict(cx=cx, cy=cy, kind=kind, rgb=(int(R), int(G), int(B)), orange=orange, yellow=yellow, star=star, grey=greyi)

# Zones from kind and position.
def zone_of(c):
    k, x, y = c["kind"], c["cx"], c["cy"]
    if k == "sea" or k == "other": return "~"
    if k == "pale": return "B" if x < W / 2 else "D"
    if k == "green": return "F" if y < H / 2 else "G"
    if k == "brown":
        if x < 620 and y < 560: return "A"
        if x > 1000 and y > 1080: return "E"
        return "C"
    return None  # rock: fill from neighbours
for c in cells.values(): c["zone"] = zone_of(c)
def neigh(q, r): return [(q+1,r),(q-1,r),(q,r+1),(q,r-1),(q+1,r-1),(q-1,r+1)]
for _ in range(3):
    for (q, r), c in cells.items():
        if c["zone"] is None:
            zs = [cells[n]["zone"] for n in neigh(q, r) if n in cells and cells[n]["zone"] not in (None, "~")]
            if zs: c["zone"] = Counter(zs).most_common(1)[0][0]
for c in cells.values():
    if c["zone"] is None: c["zone"] = "~"

# Features.
for c in cells.values():
    z, k = c["zone"], c["kind"]
    if z == "~": c["cell"] = "~~"; continue
    f = "."
    if k == "rock": f = "#"
    if k in ("pale", "brown"):
        if c["yellow"] > 0.10: f = "*"
        elif c["star"] > 0.10: f = "*"
        elif c["orange"] > 0.12: f = "r"   # red minion, kind to be set by hand
        elif c["grey"] > 0.12: f = "b"     # blue minion
    c["cell"] = z + f

json.dump({f"{q},{r}": c for (q, r), c in cells.items()}, open("cells.json", "w"), default=float)
print(Counter(c["cell"] for c in cells.values()))

# Overlay.
COL = {"~": (60, 120, 220), "A": (230, 100, 90), "B": (250, 220, 120), "C": (200, 140, 60), "D": (250, 220, 120), "E": (90, 120, 230), "F": (60, 200, 60), "G": (60, 200, 60)}
ov = rgb.copy(); d = ImageDraw.Draw(ov)
for (q, r), c in cells.items():
    z = c["zone"]
    d.polygon(verts(c["cx"], c["cy"]), outline=COL[z], width=2)
    f = c["cell"][1]
    label = f"{q},{r}"
    if f != "." and f != "~": label = f + " " + label
    if z != "~": d.text((c["cx"] - 14, c["cy"] - 6), label, fill=(0, 0, 0))
ov.resize((1400, 1400)).save("overlay3.png")
ov.crop((0, 0, 800, 800)).save("ov_tl.png"); ov.crop((789, 0, 1589, 800)).save("ov_tr.png")
ov.crop((0, 789, 800, 1589)).save("ov_bl.png"); ov.crop((789, 789, 1589, 1589)).save("ov_br.png")

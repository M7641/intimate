import numpy as np, json, math
from PIL import Image, ImageDraw, ImageFont
from collections import Counter
cells = {tuple(map(int, k.split(","))): v for k, v in json.load(open("cells2.json")).items()}
W = H = 1589
for c in cells.values():
    if c["kind"] in ("pale", "brown"):
        c["kind"] = "pale" if c["lum"] > 168 else "brown"
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
# Smooth the sand boundary: a sand cell takes the majority zone of its sand neighbours when outvoted.
for _ in range(2):
    new = {}
    for (q, r), c in cells.items():
        if c["zone"] in ("B", "C", "D"):
            zs = [cells[n]["zone"] for n in neigh(q, r) if n in cells and cells[n]["zone"] in ("B", "C", "D")]
            zs.append(c["zone"]); zs.append(c["zone"])
            new[(q, r)] = Counter(zs).most_common(1)[0][0]
    for k, z in new.items(): cells[k]["zone"] = z
for _ in range(3):
    for (q, r), c in cells.items():
        if c["zone"] is None:
            zs = [cells[n]["zone"] for n in neigh(q, r) if n in cells and cells[n]["zone"] not in (None, "~")]
            if zs: c["zone"] = Counter(zs).most_common(1)[0][0]
for c in cells.values():
    if c["zone"] is None: c["zone"] = "~"
print(Counter(c["zone"] for c in cells.values()))
json.dump({f"{q},{r}": c for (q, r), c in cells.items()}, open("cells3.json", "w"), default=float)
rgb = Image.open("real_board.png").convert("RGB")
PITCH = 81.8; ANG = math.radians(-15); Rr = PITCH / math.sqrt(3)
def verts(cx, cy): return [(cx + Rr * math.cos(ANG + math.radians(30 + 60*k)), cy + Rr * math.sin(ANG + math.radians(30 + 60*k))) for k in range(6)]
COL = {"~": (60, 120, 220), "A": (230, 60, 60), "B": (255, 240, 60), "C": (230, 110, 20), "D": (255, 240, 60), "E": (60, 90, 230), "F": (40, 220, 40), "G": (40, 220, 40)}
font = ImageFont.truetype("/usr/share/fonts/noto/NotoSerifKhmer-SemiCondensedSemiBold.ttf", 15)
ov = rgb.copy(); d = ImageDraw.Draw(ov)
for (q, r), c in cells.items():
    z = c["zone"]; d.polygon(verts(c["cx"], c["cy"]), outline=COL[z], width=3)
    if z != "~":
        t = ("#" if c["kind"] == "rock" else "") + f"{q},{r}"
        d.text((c["cx"] - 18, c["cy"] - 8), t, fill=(0, 0, 0), font=font)
        d.text((c["cx"] - 19, c["cy"] - 9), t, fill=(255, 255, 255), font=font)
for name, b in {"q0": (0, 0, 800, 800), "q1": (789, 0, 1589, 800), "q2": (0, 789, 800, 1589), "q3": (789, 789, 1589, 1589)}.items():
    ov.crop(b).save(f"z3_{name}.png")
boxes = {"A": (40, 200, 600, 640), "D": (960, 480, 1560, 1100), "E": (960, 1040, 1560, 1450)}
for z, b in boxes.items():
    crop = rgb.crop(b); dd = ImageDraw.Draw(crop)
    for (q, r), c in cells.items():
        if b[0] < c["cx"] < b[2] and b[1] < c["cy"] < b[3]:
            dd.polygon([(x - b[0], y - b[1]) for x, y in verts(c["cx"], c["cy"])], outline=COL[c["zone"]], width=2)
            t = ("#" if c["kind"] == "rock" else "") + f"{q},{r}"
            dd.text((c["cx"] - b[0] - 18, c["cy"] - b[1] - 8), t, fill=(255, 255, 255), font=font)
    crop.resize((crop.width * 3 // 2, crop.height * 3 // 2)).save(f"zone_{z}.png")

import json, math
from PIL import Image, ImageDraw, ImageFont
cells = {tuple(map(int, k.split(","))): v for k, v in json.load(open("cells3.json")).items()}
rgb = Image.open("real_board.png").convert("RGB")
PITCH = 81.8; ANG = math.radians(-15); Rr = PITCH / math.sqrt(3)
def verts(cx, cy): return [(cx + Rr * math.cos(ANG + math.radians(30 + 60*k)), cy + Rr * math.sin(ANG + math.radians(30 + 60*k))) for k in range(6)]
COL = {"~": (60, 120, 220), "A": (230, 60, 60), "B": (255, 240, 60), "C": (230, 110, 20), "D": (255, 240, 60), "E": (60, 90, 230), "F": (40, 220, 40), "G": (40, 220, 40)}
font = ImageFont.truetype("/usr/share/fonts/liberation/LiberationSans-Bold.ttf", 14)
boxes = {"A": (40, 160, 640, 620), "B": (60, 480, 700, 1100), "Cn": (420, 300, 1120, 800), "Cs": (420, 700, 1120, 1250), "D": (960, 480, 1560, 1100), "E": (940, 1020, 1560, 1450)}
for z, b in boxes.items():
    crop = rgb.crop(b); dd = ImageDraw.Draw(crop)
    for (q, r), c in cells.items():
        if b[0] - 40 < c["cx"] < b[2] + 40 and b[1] - 40 < c["cy"] < b[3] + 40:
            dd.polygon([(x - b[0], y - b[1]) for x, y in verts(c["cx"], c["cy"])], outline=COL[c["zone"]], width=2)
            t = ("#" if c["kind"] == "rock" else "") + f"{q},{r}"
            x, y = c["cx"] - b[0] - 16, c["cy"] - b[1] + 8
            dd.text((x + 1, y + 1), t, fill=(0, 0, 0), font=font); dd.text((x, y), t, fill=(255, 255, 255), font=font)
    crop.resize((crop.width * 3 // 2, crop.height * 3 // 2)).save(f"read_{z}.png")

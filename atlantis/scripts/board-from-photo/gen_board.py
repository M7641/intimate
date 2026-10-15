import json
from collections import Counter
cells = {tuple(map(int, k.split(","))): v for k, v in json.load(open("cells3.json")).items()}

# Hand-read from the labelled crops. Lattice axial coordinates.
A_PLAY = {(2,4),(3,4),(0,5),(1,5),(2,5),(-3,6),(-2,6),(-1,6),(0,6),(1,6),(-4,7),(-3,7),(-2,7),(-1,7),(0,7)}
E_PLAY = {(0,18),(-1,19),(0,19),(1,19),(2,19),(3,19),(-2,20),(-1,20),(0,20),(1,20),(2,20),(3,20),(-3,21),(-2,21)}
HERO = {"A": [(1,5),(-1,6),(-3,6)], "E": [(0,20),(2,20),(-2,21)]}
SPAWN = {
 "B": {"m": [(-5,8),(-3,9),(-2,9),(-8,14)], "h": [(-5,10)], "s": [(-6,11)],
       "M": [(-3,10),(-6,12),(-7,14)], "S": [(-4,12)], "H": [(-5,11)]},
 "C": {"m": [(2,9),(2,10),(-1,14),(-5,14)], "h": [(-4,16)], "s": [(-1,12)],
       "M": [(0,12),(-3,16),(-3,17),(4,12)], "S": [(0,14)], "H": [(3,10)]},
 "D": {"m": [(6,12),(5,14),(2,16)], "h": [(4,15)], "s": [(3,14)],
       "M": [(7,12),(1,17),(2,17),(4,18)], "S": [(5,15)], "H": [(4,16)]},
}
feat = {}
for z, groups in SPAWN.items():
    for f, hexes in groups.items():
        for h in hexes:
            cells[h]["zone"] = z; cells[h]["kind"] = "sand"; feat[h] = f
for z, hexes in HERO.items():
    for h in hexes:
        cells[h]["zone"] = z; cells[h]["kind"] = "sand"; feat[h] = "*"
for h in A_PLAY: cells[h]["zone"] = "A"; cells[h]["kind"] = cells[h]["kind"] if h in feat else "sand"
for h in E_PLAY: cells[h]["zone"] = "E"; cells[h]["kind"] = cells[h]["kind"] if h in feat else "sand"
# Jungle hexes the colour pass misread: green under the blue throne's shadow.
for h in [(-3,20), (-4,19), (-3,19)]:
    cells[h]["zone"] = "G"; cells[h]["kind"] = "green"
for h, c in cells.items():
    if c["zone"] == "A" and h not in A_PLAY: c["kind"] = "rock"
    if c["zone"] == "E" and h not in E_PLAY: c["kind"] = "rock"

def cell(h):
    c = cells[h]; z = c["zone"]
    if z == "~": return "~~"
    if h in feat: return z + feat[h]
    return z + ("#" if c["kind"] == "rock" else ".")

qs = [q for q, _ in cells]; rs = [r for _, r in cells]
rmin = min(rs)
grid = {}
for (q, r), c in cells.items():
    rr = r - rmin
    col = q + (rr - (rr & 1)) // 2
    grid[(col, rr)] = cell((q, r))
cmin = min(c for c, _ in grid); cmax = max(c for c, _ in grid); rmax = max(r for _, r in grid)
rows = []
for r in range(rmax + 1):
    rows.append(" ".join(grid.get((c, r), "__") for c in range(cmin, cmax + 1)))
print(Counter(v for v in grid.values()))
header = '''// Single-lane map, transcribed from a top-down photo of the printed board.
//
// Hex lattice fitted to the photo (pitch 81.8 px, rotated -15 degrees), then
// each hex classified by colour: sea, sand, jungle, rock. Zone borders follow
// the sand shades (pale = outposts, brown = battlefield and thrones). Spawn
// icons were read by hand against the rulebook's icon key (page 6).
// The web viewer rotates the grid by -15 degrees so it looks like the photo.
//
// Cell = zone letter + feature. Feature legend in core/src/board.rs.
// `~~` is sea (terrain), `__` is outside the photo. Rows use odd-r offset.
//
// Lane, red to blue: A Red Throne (north-west), B Red Outpost (west),
// C Battlefield (centre), D Blue Outpost (east), E Blue Throne (south-east).
// Jungles: F north, G south.
(
    zones: [
        (letter: 'A', name: "Red Throne", kind: Throne(Red)),
        (letter: 'B', name: "Red Outpost", kind: Lane),
        (letter: 'C', name: "Battlefield", kind: Lane),
        (letter: 'D', name: "Blue Outpost", kind: Lane),
        (letter: 'E', name: "Blue Throne", kind: Throne(Blue)),
        (letter: 'F', name: "North Jungle", kind: Side),
        (letter: 'G', name: "South Jungle", kind: Side),
        (letter: '~', name: "Sea", kind: Sea),
    ],
    rows: [
'''
body = "".join(("        " if i % 2 == 0 else "         ") + '"' + row + '",\n' for i, row in enumerate(rows))
open("/home/mike/github/intimate/atlantis/data/board.ron", "w").write(header + body + "    ],\n)\n")
print(len(rows), "rows,", cmax - cmin + 1, "cols")

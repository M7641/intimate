import numpy as np
from PIL import Image, ImageDraw, ImageFilter
im = Image.open("real_board.png").convert("L")
# Plain sand of the west outpost: hex edges are the only structure there.
box = (110, 560, 470, 1000)
patch = np.asarray(im.crop(box)).astype(float)
blur = np.asarray(im.crop(box).filter(ImageFilter.GaussianBlur(6))).astype(float)
hp = patch - blur                      # light vertex lines pop out
hp -= hp.mean()
F = np.fft.fft2(hp)
ac = np.fft.ifft2(F * np.conj(F)).real
ac = np.fft.fftshift(ac)
cy, cx = np.array(ac.shape) // 2
# Strongest peak on the horizontal axis away from the origin: the column pitch.
row = ac[cy, cx + 40: cx + 110]
w = 40 + int(np.argmax(row))
# Row pitch: peak along the vertical at half a column offset.
col = ac[cy + 40: cy + 100, cx + w // 2]
p = 40 + int(np.argmax(col))
print("pitch from texture: w=%d p=%d (ratio %.3f, ideal 0.866)" % (w, p, p / w))
# Refine with sub-pixel by parabolic fit is overkill; use the ratio ideal.
p = w * np.sqrt(3) / 2
# Phase: vertices of pointy-top hexes are bright. Score candidate (x0,y0).
full = np.asarray(im).astype(float)
fb = np.asarray(im.filter(ImageFilter.GaussianBlur(6))).astype(float)
hpf = full - fb
H, W = full.shape
R = w / np.sqrt(3)
def score(x0, y0):
    s = 0.0; n = 0
    for r in range(8, 17):
        for c in range(1, 7):
            cxh = x0 + w * (c + 0.5 * (r % 2)); cyh = y0 + p * r
            for vx, vy in ((cxh - w/2, cyh - R/2), (cxh + w/2, cyh - R/2), (cxh, cyh - R),
                           (cxh - w/2, cyh + R/2), (cxh + w/2, cyh + R/2), (cxh, cyh + R)):
                xi, yi = int(round(vx)), int(round(vy))
                if 0 <= xi < W and 0 <= yi < H:
                    s += hpf[yi, xi]; n += 1
    return s / max(n, 1)
best = max(((score(x0, y0), x0, y0) for x0 in np.arange(0, w, 1.0) for y0 in np.arange(0, p, 1.0)))
_, x0, y0 = best
print("phase x0=%.1f y0=%.1f" % (x0, y0))
np.save("lattice.npy", np.array([w, p, x0, y0]))
rgb = Image.open("real_board.png").convert("RGB")
d = ImageDraw.Draw(rgb)
for r in range(-1, int(H / p) + 2):
    for c in range(-1, int(W / w) + 2):
        cxh = x0 + w * (c + 0.5 * (r % 2)); cyh = y0 + p * r
        pts = [(cxh + R * np.cos(np.pi / 180 * (60 * i - 30)), cyh + R * np.sin(np.pi / 180 * (60 * i - 30))) for i in range(6)]
        d.polygon(pts, outline=(255, 0, 255))
rgb.crop((0, 400, 800, 1200)).save("overlay2.png")

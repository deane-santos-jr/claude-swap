from PIL import Image, ImageDraw
import math, sys, os

SIZE = 1024
img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
d = ImageDraw.Draw(img)

pad = 90
d.rounded_rectangle([pad, pad, SIZE - pad, SIZE - pad], radius=210, fill=(217, 119, 87, 255))

cx, cy, r = SIZE // 2, SIZE // 2, 300
stroke = 74

def arc_arrow(start, end, color, tip_angle):
    d.arc([cx - r, cy - r, cx + r, cy + r], start=start, end=end, fill=color, width=stroke)
    a = math.radians(tip_angle)
    tx, ty = cx + r * math.cos(a), cy + r * math.sin(a)
    tangent = a + math.pi / 2
    size = 150
    p1 = (tx + size * math.cos(tangent), ty + size * math.sin(tangent))
    base = (tx - 0.35 * size * math.cos(tangent), ty - 0.35 * size * math.sin(tangent))
    n = tangent + math.pi / 2
    p2 = (base[0] + 0.55 * size * math.cos(n), base[1] + 0.55 * size * math.sin(n))
    p3 = (base[0] - 0.55 * size * math.cos(n), base[1] - 0.55 * size * math.sin(n))
    d.polygon([p1, p2, p3], fill=color)

white = (255, 250, 245, 255)
cream = (255, 230, 215, 255)
arc_arrow(200, 320, white, 320)
arc_arrow(20, 140, cream, 140)

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
for px in (16, 32, 64, 128, 256, 512, 1024):
    img.resize((px, px), Image.LANCZOS).save(f"{out}/icon_{px}.png")

"""Generate the woGAer liquid-glass app icon."""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont


W = H = 1024
RADIUS = 216
ROOT = Path(__file__).resolve().parent.parent


def lerp(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def rounded_mask(size, radius):
    mask = Image.new("L", size, 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle([0, 0, size[0] - 1, size[1] - 1], radius=radius, fill=255)
    return mask


def build_icon():
    icon = Image.new("RGBA", (W, H), (0, 0, 0, 0))

    # Base liquid gradient.
    base = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    draw = ImageDraw.Draw(base)
    top = (74, 209, 232)
    bottom = (58, 92, 205)
    for y in range(H):
        t = y / (H - 1)
        draw.line([(0, y), (W, y)], fill=(*lerp(top, bottom, t), 255))

    # Soft radial glow behind the center.
    glow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    glow_draw.ellipse([150, 150, 874, 874], fill=(255, 255, 255, 42))
    glow = glow.filter(ImageFilter.GaussianBlur(60))
    base = Image.alpha_composite(base, glow)

    # Top-left light streak.
    streak = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    streak_draw = ImageDraw.Draw(streak)
    streak_draw.polygon(
        [(-160, 300), (420, -160), (700, -160), (160, 520)],
        fill=(255, 255, 255, 72),
    )
    streak = streak.filter(ImageFilter.GaussianBlur(34))
    base = Image.alpha_composite(base, streak)

    # Bottom inner shadow for glass depth.
    shadow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    shadow_draw = ImageDraw.Draw(shadow)
    shadow_draw.ellipse([-140, 620, 1164, 1300], fill=(6, 18, 74, 150))
    shadow = shadow.filter(ImageFilter.GaussianBlur(70))
    base = Image.alpha_composite(base, shadow)

    # Glass border.
    border = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    border_draw = ImageDraw.Draw(border)
    border_draw.rounded_rectangle(
        [26, 26, W - 27, H - 27],
        radius=RADIUS - 18,
        outline=(255, 255, 255, 180),
        width=10,
    )
    border_draw.rounded_rectangle(
        [52, 52, W - 53, H - 53],
        radius=RADIUS - 44,
        outline=(255, 255, 255, 70),
        width=3,
    )
    border = border.filter(ImageFilter.GaussianBlur(1))
    base = Image.alpha_composite(base, border)

    # "GA" text with a glassy embossed look.
    ga = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ga_draw = ImageDraw.Draw(ga)
    font = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 360)
    text = "GA"
    box = ga_draw.textbbox((0, 0), text, font=font)
    tw, th = box[2] - box[0], box[3] - box[1]
    cx, cy = W // 2, 455
    pos = (cx - tw // 2 - box[0], cy - th // 2 - box[1])
    ga_draw.text(
        (pos[0] + 10, pos[1] + 16),
        text,
        font=font,
        fill=(12, 40, 96, 210),
    )
    ga_draw.text(
        (pos[0], pos[1]),
        text,
        font=font,
        fill=(255, 255, 255, 255),
        stroke_width=6,
        stroke_fill=(38, 84, 158, 255),
    )
    base = Image.alpha_composite(base, ga)

    # Small brand label.
    label = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    label_draw = ImageDraw.Draw(label)
    label_font = ImageFont.truetype(
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf", 88
    )
    brand = "wogaer"
    box = label_draw.textbbox((0, 0), brand, font=label_font)
    tw, th = box[2] - box[0], box[3] - box[1]
    pos = (W // 2 - tw // 2 - box[0], 700 - th // 2 - box[1])
    label_draw.text(
        (pos[0], pos[1] + 5),
        brand,
        font=label_font,
        fill=(10, 35, 90, 190),
    )
    label_draw.text(
        (pos[0], pos[1]),
        brand,
        font=label_font,
        fill=(245, 252, 255, 255),
    )
    base = Image.alpha_composite(base, label)

    # Cut to rounded square.
    icon = Image.composite(base, icon, rounded_mask((W, H), RADIUS))
    icon.save(ROOT / "src-tauri" / "app-icon.png", "PNG")


if __name__ == "__main__":
    build_icon()

#!/usr/bin/env python3
"""Generate 16x16 Minecraft-style block textures."""

from __future__ import annotations

import random
from pathlib import Path
from typing import Iterable

from PIL import Image

SIZE = 16
OUT_DIR = Path("/Users/f37/docs/minecraft/veldspar/assets/textures/blocks")


def clamp(value: int, lo: int = 0, hi: int = 255) -> int:
    return max(lo, min(hi, value))


def jitter(color: tuple[int, int, int], amount: int, rng: random.Random) -> tuple[int, int, int]:
    return tuple(clamp(channel + rng.randint(-amount, amount)) for channel in color)


def blend(
    c1: tuple[int, int, int], c2: tuple[int, int, int], t: float
) -> tuple[int, int, int]:
    return tuple(clamp(int(a + (b - a) * t)) for a, b in zip(c1, c2))


def stone_base(
    rng: random.Random,
    base: tuple[int, int, int],
    variation: int = 12,
    dark_fleck_rate: float = 0.12,
    light_fleck_rate: float = 0.08,
) -> Image.Image:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 255))
    px = img.load()
    for y in range(SIZE):
        for x in range(SIZE):
            color = list(jitter(base, variation, rng))
            if rng.random() < dark_fleck_rate:
                color = [clamp(c - rng.randint(8, 20)) for c in color]
            elif rng.random() < light_fleck_rate:
                color = [clamp(c + rng.randint(8, 20)) for c in color]
            px[x, y] = (*color, 255)
    return img


def add_ore_speckles(
    img: Image.Image,
    rng: random.Random,
    ore_dark: tuple[int, int, int],
    ore_mid: tuple[int, int, int],
    ore_light: tuple[int, int, int],
    cluster_count: int = 20,
) -> None:
    px = img.load()
    for _ in range(cluster_count):
        cx = rng.randint(1, SIZE - 2)
        cy = rng.randint(1, SIZE - 2)
        shape = rng.choice(
            [
                [(0, 0)],
                [(0, 0), (1, 0)],
                [(0, 0), (0, 1)],
                [(0, 0), (1, 0), (0, 1)],
                [(0, 0), (-1, 0), (0, 1)],
            ]
        )
        for ox, oy in shape:
            x = clamp(cx + ox, 0, SIZE - 1)
            y = clamp(cy + oy, 0, SIZE - 1)
            pick = rng.random()
            if pick < 0.20:
                color = ore_dark
            elif pick < 0.80:
                color = ore_mid
            else:
                color = ore_light
            px[x, y] = (*color, 255)
            if x + 1 < SIZE and rng.random() < 0.35:
                px[x + 1, y] = (*ore_light, 255)
            if y + 1 < SIZE and rng.random() < 0.30:
                px[x, y + 1] = (*ore_dark, 255)


def add_cracks(img: Image.Image, rng: random.Random, count: int = 4) -> None:
    px = img.load()
    for _ in range(count):
        x = rng.randint(2, SIZE - 3)
        y = rng.randint(1, SIZE - 2)
        length = rng.randint(5, 10)
        for _step in range(length):
            if 0 <= x < SIZE and 0 <= y < SIZE:
                px[x, y] = (26, 28, 31, 255)
                if x + 1 < SIZE and rng.random() < 0.25:
                    px[x + 1, y] = (60, 62, 66, 255)
            x += rng.choice([-1, 0, 1])
            y += rng.choice([0, 1])
            x = clamp(x, 0, SIZE - 1)
            y = clamp(y, 0, SIZE - 1)


def make_ore_texture(
    seed: int,
    base: tuple[int, int, int],
    ore_dark: tuple[int, int, int],
    ore_mid: tuple[int, int, int],
    ore_light: tuple[int, int, int],
    crack_count: int,
) -> Image.Image:
    rng = random.Random(seed)
    img = stone_base(rng, base=base, variation=11)
    add_cracks(img, rng, count=crack_count)
    add_ore_speckles(img, rng, ore_dark, ore_mid, ore_light, cluster_count=22)
    return img


def clay_deposit_texture(seed: int) -> Image.Image:
    rng = random.Random(seed)
    base_a = (152, 144, 134)
    base_b = (166, 156, 146)
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 255))
    px = img.load()
    for y in range(SIZE):
        t = y / (SIZE - 1)
        row_base = blend(base_a, base_b, t * 0.35)
        stripe = -7 if y % 4 in (1, 2) else 5
        for x in range(SIZE):
            noise = rng.randint(-5, 5)
            drift = ((x + y) % 5) - 2
            r, g, b = row_base
            px[x, y] = (
                clamp(r + stripe + noise + drift),
                clamp(g + stripe + noise // 2),
                clamp(b + stripe + noise // 3),
                255,
            )
    return img


def mossy_rubble_texture(seed: int) -> Image.Image:
    rng = random.Random(seed)
    img = stone_base(rng, base=(104, 110, 106), variation=14, dark_fleck_rate=0.15, light_fleck_rate=0.10)
    px = img.load()

    # Cobblestone seams.
    seam_lines: list[list[tuple[int, int]]] = []
    for _ in range(5):
        x = rng.randint(1, SIZE - 2)
        y = rng.randint(0, SIZE - 1)
        line: list[tuple[int, int]] = []
        for _step in range(rng.randint(10, 18)):
            line.append((x, y))
            x = clamp(x + rng.choice([-1, 0, 1]), 0, SIZE - 1)
            y = clamp(y + rng.choice([-1, 0, 1]), 0, SIZE - 1)
        seam_lines.append(line)

    for line in seam_lines:
        for x, y in line:
            px[x, y] = (47, 51, 49, 255)
            if x + 1 < SIZE and rng.random() < 0.4:
                px[x + 1, y] = (124, 129, 124, 255)
            if y + 1 < SIZE and rng.random() < 0.3:
                px[x, y + 1] = (87, 93, 88, 255)

    # Moss patches.
    moss_colors = [(65, 102, 55), (78, 121, 64), (92, 136, 74)]
    for _ in range(7):
        cx = rng.randint(1, SIZE - 2)
        cy = rng.randint(1, SIZE - 2)
        points: Iterable[tuple[int, int]] = [
            (cx, cy),
            (cx + 1, cy),
            (cx, cy + 1),
            (cx - 1, cy),
            (cx, cy - 1),
        ]
        for x, y in points:
            if 0 <= x < SIZE and 0 <= y < SIZE and rng.random() < 0.75:
                px[x, y] = (*rng.choice(moss_colors), 255)
                if y + 1 < SIZE and rng.random() < 0.35:
                    px[x, y + 1] = (49, 81, 43, 255)
    return img


def tall_grass_texture(seed: int) -> Image.Image:
    rng = random.Random(seed)
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    px = img.load()
    blades = rng.randint(6, 8)
    for _ in range(blades):
        x = rng.randint(1, SIZE - 2)
        base_y = SIZE - 1
        height = rng.randint(6, 12)
        lean = rng.choice([-1, 0, 1])
        for i in range(height):
            y = base_y - i
            bx = clamp(x + (i // 3) * lean, 0, SIZE - 1)
            shade_t = i / max(1, height - 1)
            color = blend((58, 112, 44), (136, 186, 79), shade_t)
            px[bx, y] = (*color, 255)
            if rng.random() < 0.20 and bx + 1 < SIZE:
                side = blend((48, 96, 38), (118, 170, 73), shade_t)
                px[bx + 1, y] = (*side, 255)
    return img


def wildflower_texture(seed: int) -> Image.Image:
    rng = random.Random(seed)
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    px = img.load()

    stem_x = rng.randint(7, 8)
    stem_top = rng.randint(5, 7)
    for y in range(SIZE - 1, stem_top - 1, -1):
        x = stem_x + (1 if (y % 4 == 0 and rng.random() < 0.4) else 0)
        color = blend((44, 104, 41), (98, 156, 72), (SIZE - 1 - y) / 10.0)
        px[clamp(x, 0, SIZE - 1), y] = (*color, 255)

    # Small leaves.
    for lx, ly in [(stem_x - 1, 11), (stem_x + 1, 9)]:
        if 0 <= lx < SIZE and 0 <= ly < SIZE:
            px[lx, ly] = (76, 142, 63, 255)
            if lx + 1 < SIZE and rng.random() < 0.5:
                px[lx + 1, ly] = (64, 128, 56, 255)

    center = (stem_x, stem_top)
    px[center[0], center[1]] = (242, 215, 106, 255)

    petal_palette = [(212, 70, 70), (241, 206, 82), (226, 125, 171)]
    petal_offsets = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1)]
    for idx, (ox, oy) in enumerate(petal_offsets):
        x = center[0] + ox
        y = center[1] + oy
        if 0 <= x < SIZE and 0 <= y < SIZE:
            color = petal_palette[idx % len(petal_palette)]
            px[x, y] = (*color, 255)
    return img


def save_texture(name: str, img: Image.Image) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    img.save(OUT_DIR / name, format="PNG")


def main() -> None:
    textures = {
        "coal_vein.png": make_ore_texture(
            seed=1001,
            base=(98, 98, 100),
            ore_dark=(14, 14, 15),
            ore_mid=(28, 29, 31),
            ore_light=(52, 54, 56),
            crack_count=3,
        ),
        "copper_vein.png": make_ore_texture(
            seed=1002,
            base=(101, 103, 106),
            ore_dark=(112, 65, 37),
            ore_mid=(153, 89, 49),
            ore_light=(198, 127, 72),
            crack_count=4,
        ),
        "gold_vein.png": make_ore_texture(
            seed=1003,
            base=(101, 103, 106),
            ore_dark=(132, 106, 27),
            ore_mid=(177, 146, 38),
            ore_light=(227, 196, 84),
            crack_count=4,
        ),
        "diamond_vein.png": make_ore_texture(
            seed=1004,
            base=(99, 102, 105),
            ore_dark=(62, 135, 145),
            ore_mid=(88, 178, 190),
            ore_light=(159, 229, 235),
            crack_count=3,
        ),
        "tall_grass.png": tall_grass_texture(seed=2001),
        "wildflower.png": wildflower_texture(seed=2002),
        "clay_deposit.png": clay_deposit_texture(seed=3001),
        "mossy_rubble.png": mossy_rubble_texture(seed=4001),
    }

    for filename, image in textures.items():
        save_texture(filename, image)
        print(f"wrote {OUT_DIR / filename}")


if __name__ == "__main__":
    main()

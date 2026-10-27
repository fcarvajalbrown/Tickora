import argparse
from pathlib import Path

from PIL import Image, ImageDraw


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Lay out the colour map of each fetched CC0 material in one labelled sheet, optionally at a PSX texel size.")
    parser.add_argument("assets", nargs="+")
    parser.add_argument("--root", type=Path, default=Path("work/cc0"))
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--tile", type=int, default=256)
    parser.add_argument("--texels", type=int, default=0, help="downsample to this size first, then scale up nearest")
    parser.add_argument("--columns", type=int, default=4)
    return parser.parse_args(argv)


def colour_map(root, asset):
    found = sorted((root / asset).glob("*_Color.*"))
    if not found:
        raise SystemExit(f"{asset}: no colour map in {root / asset}")
    return Image.open(found[0]).convert("RGB")


def tile(image, size, texels):
    if texels > 0:
        image = image.resize((texels, texels), Image.Resampling.LANCZOS)
        return image.resize((size, size), Image.Resampling.NEAREST)
    return image.resize((size, size), Image.Resampling.LANCZOS)


def build(assets, root, size, texels, columns):
    rows = (len(assets) + columns - 1) // columns
    label = 18
    sheet = Image.new("RGB", (columns * size, rows * (size + label)), (20, 20, 20))
    draw = ImageDraw.Draw(sheet)
    for index, asset in enumerate(assets):
        x = (index % columns) * size
        y = (index // columns) * (size + label)
        sheet.paste(tile(colour_map(root, asset), size, texels), (x, y + label))
        draw.text((x + 4, y + 3), asset, fill=(230, 230, 230))
    return sheet


def main(argv=None):
    args = parse_args(argv)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    build(args.assets, args.root, args.tile, args.texels, args.columns).save(args.out)
    print(args.out)


if __name__ == "__main__":
    main()

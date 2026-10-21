"""Recolour the original menu icon without altering its shape or alpha."""
import argparse
import base64
from pathlib import Path
import re

parser = argparse.ArgumentParser()
parser.add_argument("output", type=Path)
parser.add_argument("--color", required=True, help="foreground hex colour, e.g. #ebdbb2")
args = parser.parse_args()
if not re.fullmatch(r"#[0-9a-fA-F]{6}", args.color):
    parser.error("--color must be #rrggbb")
source = Path(__file__).resolve().parents[1] / "assets/wakatime-menu.png"
encoded = base64.b64encode(source.read_bytes()).decode("ascii")
red, green, blue = (int(args.color[i:i + 2], 16) / 255 for i in (1, 3, 5))
args.output.write_text(
    '<svg xmlns="http://www.w3.org/2000/svg" '
    'xmlns:xlink="http://www.w3.org/1999/xlink" width="64" height="64" viewBox="0 0 64 64">'
    '<defs><filter id="foreground" color-interpolation-filters="sRGB">'
    f'<feColorMatrix type="matrix" values="0 0 0 0 {red} 0 0 0 0 {green} '
    f'0 0 0 0 {blue} 0 0 0 1 0"/></filter></defs>'
    f'<image width="64" height="64" filter="url(#foreground)" '
    f'xlink:href="data:image/png;base64,{encoded}"/></svg>\n'
)

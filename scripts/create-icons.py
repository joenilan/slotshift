"""Rebuild Slotshift's original geometric app icons (developer tool; Pillow required)."""
from pathlib import Path
from PIL import Image, ImageDraw
root = Path(__file__).resolve().parent.parent / "assets"
root.mkdir(exist_ok=True)
scale = 4
image = Image.new("RGBA", (256 * scale, 256 * scale))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((0, 0, 256*scale-1, 256*scale-1), radius=40*scale, fill="#141719")
draw.rounded_rectangle((45*scale, 65*scale, 169*scale, 100*scale), radius=3*scale, fill="#B2E5A0")
draw.rounded_rectangle((87*scale, 142*scale, 211*scale, 177*scale), radius=3*scale, fill="#B2E5A0")
image = image.resize((256, 256), Image.Resampling.LANCZOS)
image.save(root / "slotshift.png")
image.save(root / "slotshift.ico", sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
(root / "slotshift.svg").write_text('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" rx="40" fill="#141719"/><path d="M45 65h124v35H45zm42 77h124v35H87z" fill="#b2e5a0"/></svg>\n', encoding="utf-8")
print("Icons created.")
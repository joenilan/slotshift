"""Collect published Rust crate notices. Never inspect account data or copy fonts."""
from pathlib import Path
import json, os, subprocess

ROOT = Path(__file__).resolve().parents[1]
candidate = Path.home() / ".cargo" / "bin" / ("cargo.exe" if os.name == "nt" else "cargo")
cargo = str(candidate) if candidate.is_file() else "cargo"
result = subprocess.run([cargo, "metadata", "--locked", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], cwd=ROOT, capture_output=True, check=True)
metadata = json.loads(result.stdout)
resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
parts = ["SLOTSHIFT THIRD-PARTY SOFTWARE NOTICES\n\nGenerated from Cargo.lock and published crate packages. Optional/build dependencies may also be listed.\n"]
missing = []
packages = [p for p in metadata["packages"] if p["id"] in resolved and p["name"] != "slotshift"]
for p in sorted(packages, key=lambda p: (p["name"], p["version"])):
    base = Path(p["manifest_path"]).parent
    parts.append("\n" + "=" * 78 + f"\n{p['name']} {p['version']}\nDeclared license: {p.get('license') or 'See supplied license file'}\nUpstream: {p.get('repository') or p.get('homepage') or 'https://crates.io/crates/' + p['name']}\n")
    candidates = set()
    for folder in [base, base / "licenses", base / "LICENSES"]:
        if folder.is_dir():
            for item in folder.iterdir():
                if item.is_file() and item.name.lower().startswith(("license", "licence", "copying", "notice", "copyright")):
                    candidates.add(item)
    if p.get("license_file"):
        path = Path(p["license_file"])
        candidates.add(path if path.is_absolute() else base / path)
    found = False
    for path in sorted(candidates):
        if path.is_file() and path.stat().st_size < 2_000_000:
            try:
                text = path.read_text(encoding="utf-8-sig")
            except UnicodeError:
                continue
            if "\x00" in text:
                continue
            parts.append(f"\n--- {path.name} ---\n{text}\n")
            found = True
    if not found:
        authors = "; ".join(p.get("authors") or [])
        parts.append(f"\nPackage attribution: {authors or p['name'] + ' contributors'}\nThe published crate does not include a standalone license/notice file. Consult the declared license and upstream repository.\n")
        missing.append(p["name"] + " " + p["version"])
output = ROOT / "dist" / "THIRD_PARTY_LICENSES.txt"
output.parent.mkdir(exist_ok=True)
output.write_text("\n".join(parts), encoding="utf-8")
print(f"Cataloged {len(packages)} resolved packages; {len(missing)} without standalone packaged notice files.")
print("No-standalone-notice packages: " + ", ".join(missing))
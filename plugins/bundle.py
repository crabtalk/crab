#!/usr/bin/env python3
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

source = Path(__file__).resolve().parent / "crabtalk"
output = source.with_suffix(".zip")

with ZipFile(output, "w", compression=ZIP_DEFLATED) as archive:
    for path in sorted(source.rglob("*")):
        relative = path.relative_to(source)
        if any(part.startswith(".") for part in relative.parts):
            continue
        if path.is_symlink():
            raise SystemExit(f"Cannot bundle a symlink: {path}")
        if path.is_file():
            archive.write(path, Path(source.name) / relative)

print(output)

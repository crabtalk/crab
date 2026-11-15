#!/usr/bin/env python3
from pathlib import Path
from shutil import copy2, rmtree
from zipfile import ZIP_DEFLATED, ZipFile

source = Path(__file__).resolve().parent / "crabtalk"
skill = source.parents[1] / "skills" / "crab"
package = source.parent / "build" / source.name
output = source.with_suffix(".zip")
manifests = {Path(".codex-plugin/plugin.json"), Path(".mcp.json")}

if not (skill / "SKILL.md").is_file():
    raise SystemExit(f"Missing canonical skill: {skill}")

if package.exists():
    rmtree(package)
for directory, prefix in ((source, Path()), (skill, Path("skills/crab"))):
    for path in sorted(directory.rglob("*")):
        relative = prefix / path.relative_to(directory)
        if relative not in manifests and any(part.startswith(".") for part in relative.parts):
            continue
        if path.is_symlink():
            raise SystemExit(f"Cannot bundle a symlink: {path}")
        if path.is_file():
            destination = package / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            copy2(path, destination)

with ZipFile(output, "w", compression=ZIP_DEFLATED) as archive:
    for path in sorted(package.rglob("*")):
        if path.is_file():
            archive.write(path, Path(source.name) / path.relative_to(package))

print(output)

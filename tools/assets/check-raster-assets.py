"""Reject vector image assets anywhere in the working tree, including ignored files."""
from pathlib import Path
import argparse
import os
import re


ROOT = Path(__file__).resolve().parents[2]
VECTOR_CONTENT = re.compile(rb"<svg(?:\s|>)|image/svg\+xml", re.IGNORECASE)


def violations(root: Path) -> list[str]:
    found = []
    for folder, directories, files in os.walk(root, followlinks=False):
        # Git's immutable object history is not a working-tree asset provider.
        directories[:] = [name for name in directories if name != ".git"]
        for name in files:
            path = Path(folder) / name
            relative = path.relative_to(root)
            if path.suffix.lower() in (".svg", ".svgz"):
                found.append(f"{relative}: vector image file")
            elif relative.parts[0] in ("assets", "src") and VECTOR_CONTENT.search(path.read_bytes()):
                found.append(f"{relative}: embedded vector image content")
    return sorted(found)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    found = violations(args.root.resolve(strict=True))
    if found:
        print("\n".join(found))
        return 1
    print("PASS: no SVG files in the working tree or embedded SVG in assets/source.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

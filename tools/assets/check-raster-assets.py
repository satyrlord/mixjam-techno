"""Reject vector assets in public repo files; respect private project ignore rules."""
from pathlib import Path
import argparse
import re
import subprocess


ROOT = Path(__file__).resolve().parents[2]
VECTOR_CONTENT = re.compile(rb"<svg(?:\s|>)|image/svg\+xml", re.IGNORECASE)


def violations(root: Path) -> list[str]:
    found = []
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        check=True, capture_output=True,
    )
    for name in sorted(set(result.stdout.decode("utf-8").split("\0")) - {""}):
        relative = Path(name)
        path = root / relative
        # A tracked deletion is not an asset in the current checkout.
        if not path.is_file():
            continue
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
    print("PASS: no SVG files in tracked/non-ignored project files or embedded SVG in application assets/source.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

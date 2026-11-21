"""Reject vector assets in public repo files; respect private project ignore rules."""
from pathlib import Path
import argparse
from codecs import BOM_UTF16_BE, BOM_UTF16_LE, BOM_UTF32_BE, BOM_UTF32_LE
from io import TextIOWrapper
import re
import subprocess


ROOT = Path(__file__).resolve().parents[2]
VECTOR_CONTENT = re.compile(r"<svg(?:\s|[>/])|data:image/svg\+xml[;,]", re.IGNORECASE | re.ASCII)
CHUNK = 1 << 20
OVERLAP = 32  # longer than the longest match, so a match across a chunk edge is found


def has_vector_content(path: Path) -> bool:
    """Whether the file holds vector markup; it reads the file in chunks, not whole."""
    kept = ""
    with path.open("rb") as raw:
        prefix = raw.peek(4)
        if prefix.startswith((BOM_UTF32_BE, BOM_UTF32_LE)):
            encoding = "utf-32"
        elif prefix.startswith((BOM_UTF16_BE, BOM_UTF16_LE)):
            encoding = "utf-16"
        else:
            encoding = "utf-8"
        with TextIOWrapper(raw, encoding=encoding, errors="replace") as stream:
            while chunk := stream.read(CHUNK):
                window = kept + chunk
                if VECTOR_CONTENT.search(window):
                    return True
                kept = window[-OVERLAP:]
    return False


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
        elif not path.is_symlink() and has_vector_content(path):
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
    print("PASS: no SVG files or embedded SVG in tracked/non-ignored project files.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

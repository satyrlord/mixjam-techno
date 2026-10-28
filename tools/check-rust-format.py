"""Find Rust source that `cargo fmt --check` does not cover.

Stable rustfmt leaves a statement as written when it cannot fit the statement
in the line width, for example around a long string literal, and does not
report it. It also never visits a file that reaches the crate only through
`include!`. For each tracked or new .rs file, this check formats the file on
its own and must get the file back unchanged. It then changes the whitespace
between tokens and formats again. A change that survives marks a region that
rustfmt skips. Code that rustfmt skips without a width limit, such as a
`json!` object, is not reported. Split a long literal with `\\` line
continuations so that rustfmt can format the statement.
"""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import argparse
import difflib
import os
import re
import shutil
import subprocess


ROOT = Path(__file__).resolve().parents[1]
IDENT = re.compile(r"r#[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_0-9][A-Za-z0-9_]*")
RAW_START = re.compile(r'(#*)"')
EDITION = re.compile(r'(?m)^edition\s*=\s*"(\d+)"')
UNLIMITED = ("--config", "max_width=10000")
SPACEABLE = set("()[]{},;")
LOOKAHEAD = 16


def tokens(text: str) -> list[tuple[int, int]]:
    """Split Rust source into token spans; a comment or a literal is one token."""
    found = []
    index = 0

    def quoted(body: int, quote: str = '"') -> int:
        while text[body] != quote:
            body += 2 if text[body] == "\\" else 1
        return body + 1

    while index < len(text):
        char = text[index]
        if char.isspace():
            index += 1
            continue
        start = index
        if text.startswith("//", index):
            end = text.find("\n", index)
            index = len(text) if end < 0 else end
        elif text.startswith("/*", index):
            depth, index = 1, index + 2
            while depth:
                if text.startswith("/*", index):
                    depth, index = depth + 1, index + 2
                elif text.startswith("*/", index):
                    depth, index = depth - 1, index + 2
                else:
                    index += 1
        elif char == '"':
            index = quoted(index + 1)
        elif char == "'" and text[index + 1] == "\\":
            index = quoted(index + 1, "'")
        elif char == "'" and text[index + 2:index + 3] == "'":
            index += 3
        elif match := IDENT.match(text, index):
            word, index = match.group(), match.end()
            raw = RAW_START.match(text, index)
            if word in ("r", "br", "cr") and raw:
                closing = '"' + raw.group(1)
                index = text.index(closing, raw.end()) + len(closing)
            elif word in ("b", "c") and text.startswith('"', index):
                index = quoted(index + 1)
            elif word == "b" and text.startswith("'", index):
                index = quoted(index + 1, "'")
        else:
            index += 1
        found.append((start, index))
    return found


def shifted(text: str, spans: list[tuple[int, int]], keep: set[int] = frozenset()) -> str:
    """Change the whitespace before each token not in `keep`.

    A token that starts a line loses one space of indentation, so that its line
    cannot grow past the width. Another token gets one more space before it.
    Tokens with no gap get one only next to a bracket, a comma, or a semicolon,
    where a space cannot join or split an operator, a number, or a lifetime.
    rustfmt undoes each change where it formats the code.
    """
    out, previous = [], 0
    for index, (start, end) in enumerate(spans):
        gap = text[previous:start]
        if index in keep:
            out.append(gap)
        elif "\n" in gap:
            out.append(gap[:-1] if gap.endswith(" ") else gap)
        elif gap or (previous and SPACEABLE & {text[previous - 1], text[start]}):
            out.append(gap + " ")
        else:
            out.append(gap)
        out.append(text[start:end])
        previous = end
    out.append(text[previous:])
    return "".join(out)


def gaps(text: str, spans: list[tuple[int, int]]) -> list[str]:
    return [text[(spans[i - 1][1] if i else 0):start] for i, (start, _) in enumerate(spans)]


def align(left: list[str], right: list[str]) -> list[tuple[int, int]]:
    """Pair equal tokens of two token lists that differ by a few local edits.

    rustfmt adds or removes commas, semicolons, and braces; it does not move
    code within a formatted file.
    """
    pairs, i, j = [], 0, 0

    def resumes(a: int, b: int) -> bool:
        return left[a:a + 3] == right[b:b + 3]

    while i < len(left) and j < len(right):
        if left[i] == right[j]:
            pairs.append((i, j))
            i, j = i + 1, j + 1
            continue
        for skip in range(1, LOOKAHEAD):
            if resumes(i + skip, j):
                i += skip
                break
            if resumes(i, j + skip):
                j += skip
                break
        else:
            i, j = i + 1, j + 1
    return pairs


def kept(text: str, spans: list[tuple[int, int]], shift: str, formatted: str) -> set[int]:
    """Return the indexes of tokens whose changed gap survives formatting."""
    shift_spans, out_spans = tokens(shift), tokens(formatted)
    before, after, out = gaps(text, spans), gaps(shift, shift_spans), gaps(formatted, out_spans)
    words = [text[s:e] for s, e in spans]
    out_words = [formatted[s:e] for s, e in out_spans]
    return {i for i, j in align(words, out_words) if after[i] != before[i] and out[j] == after[i]}


def line_ranges(text: str, spans: list[tuple[int, int]], indexes: set[int]) -> list[tuple[int, int]]:
    lines = sorted({text.count("\n", 0, spans[i][0]) + 1 for i in indexes})
    ranges = []
    for line in lines:
        if ranges and line <= ranges[-1][1] + 2:
            ranges[-1][1] = line
        else:
            ranges.append([line, line])
    return [(start, end) for start, end in ranges]


def changed_lines(before: str, after: str) -> list[tuple[int, int]]:
    matcher = difflib.SequenceMatcher(a=before.split("\n"), b=after.split("\n"), autojunk=False)
    return [(a + 1, max(a + 1, b)) for tag, a, b, _, _ in matcher.get_opcodes() if tag != "equal"]


def rustfmt_command(root: Path) -> tuple[list[str], dict[str, str]]:
    """Use `RUSTFMT`, else the toolchain in local-data/, else rustfmt on PATH."""
    env = dict(os.environ)
    if "RUSTFMT" in env:
        return [env["RUSTFMT"]], env
    toolchain = root / "local-data" / "toolchain"
    for name in ("rustfmt.exe", "rustfmt"):
        proxy = toolchain / "cargo" / "bin" / name
        if proxy.is_file():
            env["CARGO_HOME"] = str(toolchain / "cargo")
            env["RUSTUP_HOME"] = str(toolchain / "rustup")
            return [str(proxy)], env
    found = shutil.which("rustfmt")
    if not found:
        raise SystemExit("rustfmt not found; run tools/runtime/install-rust.ps1 or set RUSTFMT.")
    return [found], env


def edition(path: Path) -> str:
    for folder in path.parents:
        manifest = folder / "Cargo.toml"
        if manifest.is_file():
            match = EDITION.search(manifest.read_text(encoding="utf-8"))
            return match.group(1) if match else "2015"
    return "2015"


def check(path: Path, name: str, command: list[str], env: dict[str, str]) -> list[str]:
    text = path.read_text(encoding="utf-8").replace("\r\n", "\n")

    def rustfmt(source: str, *options: str) -> str | None:
        # From the file's folder, rustfmt finds the configuration that Cargo uses.
        result = subprocess.run(
            command + ["--edition", edition(path), "--emit", "stdout", "--quiet", *options],
            input=source.encode("utf-8"), capture_output=True, env=env, cwd=path.parent,
        )
        if result.returncode != 0:
            return None
        return result.stdout.decode("utf-8").replace("\r\n", "\n")

    formatted = rustfmt(text)
    if formatted is None:
        return [f"{name}: rustfmt cannot parse the file"]
    found = [f"{name}:{a}-{b}: not formatted" for a, b in changed_lines(text, formatted)]
    if found:
        return found
    # rustfmt never formats some code, such as a `json!` object or a `matches!`
    # guard. Find it without a width limit and leave its gaps as they are, so
    # that the added spaces cannot push its lines past the width.
    spans = tokens(text)
    shift = shifted(text, spans)
    unlimited = rustfmt(shift, *UNLIMITED)
    limited = None
    if unlimited is not None:
        shift = shifted(text, spans, kept(text, spans, shift, unlimited))
        limited = rustfmt(shift)
    if limited is None:
        return [f"{name}: rustfmt cannot parse the file after the whitespace change"]
    skipped = line_ranges(text, spans, kept(text, spans, shift, limited))
    return [f"{name}:{a}-{b}: rustfmt skips this region; make each line fit" for a, b in skipped]


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("files", nargs="*", type=Path, help="files to check instead of all .rs files")
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    if args.files:
        paths = [path.resolve(strict=True) for path in args.files]
    else:
        result = subprocess.run(
            ["git", "-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard",
             "-z", "*.rs"],
            check=True, capture_output=True,
        )
        names = sorted(set(result.stdout.decode("utf-8").split("\0")) - {""})
        # A tracked deletion is not a file in the current checkout.
        paths = [root / name for name in names if (root / name).is_file()]
    command, env = rustfmt_command(root)

    def run(path: Path) -> list[str]:
        name = path.relative_to(root).as_posix() if path.is_relative_to(root) else str(path)
        return check(path, name, command, env)

    with ThreadPoolExecutor(max_workers=os.cpu_count()) as pool:
        found = [line for lines in pool.map(run, paths) for line in lines]
    if found:
        print("\n".join(found))
        return 1
    print(f"PASS: rustfmt formats every region of {len(paths)} Rust files.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

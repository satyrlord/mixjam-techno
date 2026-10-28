"""Regression checks for the Rust format check's whitespace changes."""
import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("rust_format", ROOT / "tools/check-rust-format.py")
rust_format = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rust_format)


class RustFormatTests(unittest.TestCase):
    def test_character_literals_remain_single_tokens(self):
        for literal in (r"b'\''", r"b'\\'", r"b'\x27'", "b'('", r"'\''", r"'\\'", r"'\u{27}'", "'('"):
            with self.subTest(literal=literal):
                self.assertEqual(rust_format.tokens(literal), [(0, len(literal))])

    def test_macro_with_adjacent_byte_and_character_literals_passes(self):
        # rustfmt leaves this macro's token tree unchanged. The scanner must
        # preserve both literals when it changes whitespace around the tokens.
        source = r"""macro_rules! ignore {
    ($($tokens:tt)*) => {};
}
fn main() {
    ignore! {b'\'','('=>0}
}
"""
        command, env = rust_format.rustfmt_command(ROOT)
        scratch = ROOT / "tmp"
        scratch.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="rust-format-test-", dir=scratch) as folder:
            path = Path(folder) / "fixture.rs"
            path.write_text(source, encoding="utf-8")
            self.assertEqual(rust_format.check(path, "fixture.rs", command, env), [])


if __name__ == "__main__":
    unittest.main()

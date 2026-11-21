"""Check the raster policy against tracked, new, and ignored files."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "tools/assets/check-raster-assets.py"


class RasterAssetTests(unittest.TestCase):
    def test_embedded_svg_is_checked_in_all_public_files(self):
        scratch = ROOT / "tmp"
        scratch.mkdir(exist_ok=True)
        env = {key: value for key, value in os.environ.items()
               if not key.upper().startswith("GIT_")}
        with tempfile.TemporaryDirectory(prefix="raster-assets-", dir=scratch) as folder:
            repo = Path(folder)

            def git(*args):
                subprocess.run(["git", *args], cwd=repo, env=env,
                               capture_output=True, check=True)

            git("init", "--quiet")
            (repo / "tracked.html").write_text("<main><" "svg/></main>", encoding="utf-8")
            git("add", "tracked.html")
            (repo / ".gitignore").write_text("tracked.html\nprivate.html\n", encoding="utf-8")
            (repo / "private.html").write_text("<" "svg width='20'></svg>", encoding="utf-8")
            # The opening tag crosses the read boundary of the streaming scan.
            (repo / "untracked.html").write_bytes(b" " * ((1 << 20) - 2) + b"<" b"svg></svg>")
            (repo / "wide.html").write_text("<" "svg></svg>", encoding="utf-16")
            (repo / "encoded.html").write_text(
                '<img src="data:' 'image/svg+xml;base64,PHN2Zy8+">', encoding="utf-8",
            )
            (repo / "mime.txt").write_text("The media type is image/svg+xml.", encoding="utf-8")

            checked = subprocess.run([sys.executable, str(CHECKER), "--root", str(repo)],
                                     env=env, capture_output=True, text=True)
            self.assertEqual(checked.returncode, 1, checked.stdout + checked.stderr)
            self.assertEqual(checked.stdout.splitlines(), [
                "encoded.html: embedded vector image content",
                "tracked.html: embedded vector image content",
                "untracked.html: embedded vector image content",
                "wide.html: embedded vector image content",
            ])

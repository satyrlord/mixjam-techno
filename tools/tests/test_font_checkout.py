"""Check that Git preserves pinned font bytes and the generated manifest."""
from hashlib import sha256
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FONTS = ROOT / "assets/fonts"


class FontCheckoutTests(unittest.TestCase):
    def test_font_assets_survive_git_line_ending_modes(self):
        manifest = (FONTS / "manifest.json").read_bytes()
        expected = {"manifest.json": sha256(manifest).hexdigest()}
        for record in json.loads(manifest)["files"]:
            expected[record["file"]] = record["sha256"]
            expected[record["license"]] = record["license_sha256"]
        # Keep every Git operation inside the fixture, including its index.
        env = {key: value for key, value in os.environ.items()
               if not key.upper().startswith("GIT_")}
        env["GIT_ATTR_NOSYSTEM"] = "1"
        scratch = ROOT / "tmp"
        scratch.mkdir(exist_ok=True)
        for clean in ("true", "false"):
            for checkout in ("true", "false"):
                with self.subTest(clean=clean, checkout=checkout):
                    with tempfile.TemporaryDirectory(prefix="font-checkout-", dir=scratch) as folder:
                        repo = Path(folder)

                        def git(mode, *args):
                            subprocess.run(
                                ["git", "-c", f"core.autocrlf={mode}",
                                 "-c", "core.safecrlf=false", "-c", "core.attributesFile=", *args],
                                cwd=repo, env=env, capture_output=True, check=True,
                            )

                        git(clean, "init", "--quiet")
                        attributes = ROOT / ".gitattributes"
                        if attributes.is_file():
                            shutil.copyfile(attributes, repo / ".gitattributes")
                        destination = repo / "assets/fonts"
                        destination.mkdir(parents=True)
                        for name in expected:
                            shutil.copyfile(FONTS / name, destination / name)
                        git(clean, "add", ".")
                        for name in expected:
                            (destination / name).unlink()
                        git(checkout, "checkout-index", "--all", "--force")
                        for name, digest in expected.items():
                            with self.subTest(asset=name):
                                self.assertEqual(
                                    sha256((destination / name).read_bytes()).hexdigest(), digest,
                                    f"Git changed assets/fonts/{name}",
                                )


if __name__ == "__main__":
    unittest.main()

"""Checks for the song tools' settings in tools/song/env.py."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import env  # noqa: E402


class EnvTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        patcher = mock.patch.dict(os.environ, {}, clear=False)
        patcher.start()
        self.addCleanup(patcher.stop)
        for key in ('SAMPLES_FOLDER', 'ARTIST_PROFILE', 'REFERENCE_PROFILE', 'SONG_FOLDER',
                    *(f'SAMPLES_FOLDER_{n}' for n in range(1, 10))):
            os.environ.pop(key, None)

    def tearDown(self):
        self.directory.cleanup()

    def write(self, text):
        (self.root / '.env.local').write_text(text, encoding='utf-8')

    def test_parse_reads_quoted_and_bare_values_and_skips_comments(self):
        values = env.parse('# comment\nA="x y"\nB=plain\nexport C=\'z\'\n\nnot a pair\n')
        self.assertEqual(values, {'A': 'x y', 'B': 'plain', 'C': 'z'})

    def test_environment_overrides_the_file(self):
        self.write('SAMPLES_FOLDER="from-file"\n')
        self.assertEqual(env.get('SAMPLES_FOLDER', self.root), 'from-file')
        os.environ['SAMPLES_FOLDER'] = 'from-environment'
        self.assertEqual(env.get('SAMPLES_FOLDER', self.root), 'from-environment')

    def test_missing_key_names_the_key_and_the_example_file(self):
        self.write('')
        with self.assertRaises(SystemExit) as caught:
            env.require('SAMPLES_FOLDER', self.root)
        self.assertIn('SAMPLES_FOLDER', str(caught.exception))
        self.assertIn('.env.example', str(caught.exception))

    def test_sample_folders_are_the_numbered_keys_in_order(self):
        self.write('SAMPLES_FOLDER_3="third"\nSAMPLES_FOLDER_1="first"\nSAMPLES_FOLDER_10="ignored"\n')
        self.assertEqual(env.sample_folders(self.root), [self.root / 'first', self.root / 'third'])

    def test_relative_paths_are_read_from_the_root(self):
        songs = (self.root / 'songs').as_posix()
        self.write(f'ARTIST_PROFILE="local-data/profile.md"\nSONG_FOLDER="{songs}"\n')
        self.assertEqual(env.path('ARTIST_PROFILE', root=self.root), self.root / 'local-data' / 'profile.md')
        self.assertEqual(env.path('SONG_FOLDER', root=self.root), Path(songs))
        self.assertIsNone(env.path('REFERENCE_PROFILE', required=False, root=self.root))

    def test_reference_profile_defaults_to_the_artist_profile_folder(self):
        self.write('ARTIST_PROFILE="private/profile.md"\n')
        self.assertEqual(env.reference_profile(self.root), self.root / 'private' / 'reference.json')
        self.write('ARTIST_PROFILE="private/profile.md"\nREFERENCE_PROFILE="other.json"\n')
        self.assertEqual(env.reference_profile(self.root), self.root / 'other.json')
        self.write('')
        self.assertIsNone(env.reference_profile(self.root))


if __name__ == '__main__':
    unittest.main()

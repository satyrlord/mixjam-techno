"""Regression checks for the song projection used by artifact verification."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'runtime'))
from mixjam_song import read_song


class SongProjectionTests(unittest.TestCase):
    def project(self, document):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'song.mixjam'
            path.write_text(json.dumps(document), encoding='utf-8')
            return read_song(path)

    def document(self, version=10):
        return {
            'formatVersion': version,
            'song': {'title': 'Test', 'author': '', 'bpm': 140,
                     'masterGain': 0.75, 'loopRegion': None},
            'lanes': [{'gain': 0.5, 'polarityInverted': False, 'pan': 0,
                       'leftEnabled': True, 'rightEnabled': True, 'placements': []}],
            'samples': [], 'missingSamples': [], 'sampleConflicts': [],
            'importNotices': [], 'sourceNotices': [], 'libraries': [],
        }

    def test_current_song_preserves_all_master_settings(self):
        document = self.document(12)
        master = self.project(self.document())['master']
        master['trim'] = -2.5
        master['limiter'].update(on=True, ceiling=-0.3)
        master['order'][:2] = reversed(master['order'][:2])
        document['master'] = master
        self.assertEqual(self.project(document)['master'], master)

    def test_version_11_preserves_booster_and_adds_disabled_pedals(self):
        document = self.document(11)
        defaults = self.project(self.document())['master']
        document['master'] = {key: copy.deepcopy(defaults[key])
                              for key in ('ejayEq', 'ejayCompressor', 'ejayWide')}
        document['master']['order'] = ['ejayWide', 'ejayEq', 'ejayCompressor']
        document['master']['ejayWide'].update(on=True, spread=700)
        result = self.project(document)['master']
        self.assertEqual(result['order'][:3], document['master']['order'])
        self.assertEqual(result['ejayWide'], {'on': True, 'spread': 700})
        self.assertEqual(result['trim'], 0.0)
        for key in defaults['order'][3:]:
            self.assertEqual(result[key], defaults[key])
            self.assertFalse(result[key]['on'])

    def test_earlier_songs_keep_signed_gain_and_default_mute_semantics(self):
        for version in (8, 9, 10):
            document = self.document(version)
            if version == 8:
                document['lanes'][0]['gain'] = -0.5
            project = self.project(document)
            self.assertEqual(project['tracks'][0]['gain'], 0.5)
            self.assertEqual(project['tracks'][0]['polarity_inverted'], version == 8)
            self.assertFalse(project['tracks'][0]['muted'])
            self.assertTrue(all(not project['master'][key]['on']
                                for key in project['master']['order']))

    def test_unknown_version_still_rejects(self):
        with self.assertRaises(RuntimeError):
            self.project(self.document(13))


if __name__ == '__main__':
    unittest.main()

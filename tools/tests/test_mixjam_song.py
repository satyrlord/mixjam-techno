"""Regression checks for the song projection used by artifact verification."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'runtime'))
from mixjam_song import BOOSTER_KEYS, read_song


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
        (eq, booster_eq), (compressor, booster_compressor), (wide, booster_wide) = BOOSTER_KEYS
        document['master'] = {earlier: copy.deepcopy(defaults[current])
                              for earlier, current in BOOSTER_KEYS}
        document['master']['order'] = [wide, eq, compressor]
        document['master'][wide].update(on=True, spread=700)
        result = self.project(document)['master']
        self.assertEqual(result['order'][:3], [booster_wide, booster_eq, booster_compressor])
        self.assertEqual(result[booster_wide], {'on': True, 'spread': 700})
        self.assertNotIn(wide, result)
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

    def test_version_13_keeps_sends_and_fx_buses_and_earlier_songs_default_them(self):
        document = self.document(13)
        document['master'] = self.project(self.document())['master']
        document['lanes'][0]['sends'] = [0.0, 0.25, 0.0, 1.0]
        buses = self.project(self.document())['fx']
        buses[1] = {'module': {'type': 'aetherform-reverb', 'decaySeconds': 2.8},
                    'powered': False, 'returnLevel': 0.5, 'limiter': True}
        document['fxBuses'] = buses
        project = self.project(document)
        self.assertEqual(project['tracks'][0]['sends'], [0.0, 0.25, 0.0, 1.0])
        self.assertEqual(project['fx'], buses)
        earlier = self.project(self.document(10))
        self.assertEqual(earlier['tracks'][0]['sends'], [0.0] * 4)
        self.assertTrue(all(bus['module'] == {'type': 'empty'} for bus in earlier['fx']))

    def test_version_14_keeps_fades_pitch_and_automation_and_earlier_songs_default_them(self):
        document = self.document(14)
        document['master'] = self.project(self.document())['master']
        document['fxBuses'] = self.project(self.document())['fx']
        document['lanes'][0]['sends'] = [0.0] * 4
        fades = {'in': {'ticks': 64, 'curve': 'equal-power'},
                 'out': {'ticks': 0, 'curve': 'linear'}}
        automation = {'volume': [{'tick': 0, 'value': -60.0}, {'tick': 256, 'value': 0.0}],
                      'filter': [{'tick': 512, 'value': -0.5}]}
        document['lanes'][0]['automation'] = automation
        document['lanes'][0]['placements'] = [
            {'id': 1, 'sampleId': 5, 'startTick': 0, 'durationTicks': 256,
             'offsetFrames': 0, 'gain': 1.0, 'fades': fades, 'pitchCents': -30},
            {'id': 2, 'sampleId': 5, 'startTick': 256, 'durationTicks': 256,
             'offsetFrames': 0, 'gain': 1.0}]
        project = self.project(document)
        self.assertEqual(project['tracks'][0]['automation'], automation)
        self.assertEqual((project['clips'][0]['fades'], project['clips'][0]['pitch_cents']),
                         (fades, -30))
        self.assertEqual(project['clips'][1]['pitch_cents'], 0)
        self.assertEqual(project['clips'][1]['fades']['in']['ticks'], 0)
        self.assertEqual(self.project(self.document(10))['tracks'][0]['automation'], {})

    def test_version_15_keeps_the_channel_eq_and_bypass_and_earlier_songs_default_them(self):
        document = self.document(15)
        master = self.project(self.document())['master']
        master['bypass'] = True
        document['master'] = master
        document['fxBuses'] = self.project(self.document())['fx']
        document['lanes'][0]['sends'] = [0.0] * 4
        eq = dict(self.project(self.document())['tracks'][0]['eq'], on=True, highDb=-9.0)
        document['lanes'][0]['eq'] = eq
        document['lanes'].append(copy.deepcopy(document['lanes'][0]))
        del document['lanes'][1]['eq']
        project = self.project(document)
        self.assertEqual(project['master']['bypass'], True)
        self.assertEqual(project['tracks'][0]['eq'], eq)
        self.assertFalse(project['tracks'][1]['eq']['on'])
        self.assertEqual(project['tracks'][1]['eq']['highCut'], 20000.0)
        earlier = self.project(self.document(10))
        self.assertFalse(earlier['tracks'][0]['eq']['on'])
        self.assertNotIn('bypass', earlier['master'])
        # Each lane's default is its own copy.
        earlier['tracks'][0]['eq']['on'] = True
        self.assertFalse(self.project(self.document(10))['tracks'][0]['eq']['on'])

    def test_version_16_keeps_its_booster_keys_and_earlier_songs_rename_theirs(self):
        current = self.project(self.document())['master']
        current['boosterWide'].update(on=True, spread=700)
        current['order'][:3] = ['boosterWide', 'boosterEq', 'boosterCompressor']
        renamed = {current_key: earlier for earlier, current_key in BOOSTER_KEYS}
        earlier = {renamed.get(key, key): value for key, value in current.items()}
        earlier['order'] = [renamed.get(name, name) for name in current['order']]
        for version, master in ((12, earlier), (15, earlier), (16, current)):
            document = self.document(version)
            document['master'] = copy.deepcopy(master)
            document['fxBuses'] = self.project(self.document())['fx']
            self.assertEqual(self.project(document)['master'], current, version)

    def test_unknown_version_still_rejects(self):
        with self.assertRaises(RuntimeError):
            self.project(self.document(18))

    def test_version_17_preserves_duckers_and_master_and_fx_keys(self):
        document = self.document(17)
        earlier = self.project(self.document())
        document['master'] = earlier['master']
        document['master']['busCompressor'].update(on=True, key=2)
        document['fxBuses'] = earlier['fx']
        document['fxBuses'][0]['module'] = {'type': 'echoform-delay', 'duckKey': 3}
        ducker = dict(on=True, key=2, threshold=-30.0, depth=6.0, attack=2.0, release=120.0)
        document['lanes'][0]['ducker'] = ducker
        document['lanes'].append(copy.deepcopy(document['lanes'][0]))
        del document['lanes'][1]['ducker']
        projected = self.project(document)
        self.assertEqual(projected['tracks'][0]['ducker'], ducker)
        self.assertEqual(projected['tracks'][1]['ducker'], dict(ducker, on=False, key=1))
        self.assertEqual(projected['master'], document['master'])
        self.assertEqual(projected['fx'], document['fxBuses'])
        self.assertEqual(earlier['tracks'][0]['ducker'], dict(ducker, on=False, key=1))

    def test_default_fades_are_independent_between_clips_and_reads(self):
        document = self.document(10)
        document['lanes'][0]['placements'] = [
            {'id': index, 'sampleId': 5, 'startTick': index * 256,
             'durationTicks': 256, 'offsetFrames': 0, 'gain': 1.0}
            for index in (1, 2)]
        project = self.project(document)
        project['clips'][0]['fades']['in']['ticks'] = 64
        self.assertEqual(project['clips'][1]['fades']['in']['ticks'], 0)
        self.assertEqual(self.project(document)['clips'][0]['fades']['in']['ticks'], 0)


if __name__ == '__main__':
    unittest.main()

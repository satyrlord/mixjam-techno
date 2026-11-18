"""Regression checks for phrase boundaries and harmonic evidence availability."""
from pathlib import Path
import io
import json
import sys
import tempfile
import unittest
from unittest import mock

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import clash  # noqa: E402
import mixjam  # noqa: E402
import phrase  # noqa: E402


class PhraseTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.source = Path(self.directory.name) / 'source.wav'
        self.source.touch()
        self.document = dict(song=dict(bpm=120), samples=[dict(
            id=1, name='loop', path=str(self.source), sourceBpm=120)],
            lanes=[dict(placements=[])])
        self.lanes = {'1': dict(name='lead', role='lead')}
        self.loop = mock.Mock(fpt=1, ticks=1024, bars=4, figure=1024)
        self.loop.on_edge.side_effect = lambda position: position % self.loop.figure <= phrase.SLACK \
            or position % self.loop.figure >= self.loop.figure - phrase.SLACK
        self.factory = self.enterContext(mock.patch.object(phrase, 'Loop', return_value=self.loop))

    def clip(self, start=0, duration=512, offset=0, **extra):
        placement = dict(sampleId=1, startTick=start, durationTicks=duration, offsetFrames=offset, **extra)
        self.document['lanes'][0]['placements'].append(placement)
        return placement

    def test_pitch_lane_can_play_a_whole_phrase_in_half_the_time(self):
        self.clip()
        self.document['lanes'][0]['automation'] = {'pitch': [dict(tick=0, value=1200)]}
        self.assertEqual(phrase.cuts(self.document, self.lanes), [])

    def test_slow_pitch_lane_still_reports_an_incomplete_phrase(self):
        self.clip(duration=1024)
        self.document['lanes'][0]['automation'] = {'pitch': [dict(tick=0, value=-1200)]}
        self.assertEqual([cut[-1] for cut in phrase.cuts(self.document, self.lanes)], ['end'])

    def test_reversed_slices_continue_at_their_actual_source_edges(self):
        self.clip(duration=512, offset=512, reverse=True)
        self.clip(start=512, duration=512, offset=0, reverse=True)
        self.assertEqual(phrase.cuts(self.document, self.lanes), [])

    def test_a_phrase_continues_on_its_rendered_twin_track(self):
        self.clip(duration=512)
        self.document['lanes'].append(dict(placements=[dict(
            sampleId=1, startTick=512, durationTicks=512, offsetFrames=512)]))
        self.lanes['2'] = dict(name='lead (twin)', role='lead')
        self.assertEqual(phrase.cuts(self.document, self.lanes), [])

    def test_device_exemption_requires_the_whole_clip_to_be_inside(self):
        self.clip(duration=384)
        self.lanes['1']['cuts'] = [[1, 1, 'first bar only']]
        self.assertEqual([cut[-1] for cut in phrase.cuts(self.document, self.lanes)], ['end'])

    def test_a_replaced_source_is_read_on_the_next_check(self):
        self.clip(duration=256)
        replacement = mock.Mock(fpt=1, ticks=256, bars=1, figure=256)
        replacement.on_edge.side_effect = lambda position: position % 256 <= phrase.SLACK
        self.factory.side_effect = [self.loop, replacement]
        self.assertEqual([cut[-1] for cut in phrase.cuts(self.document, self.lanes)], ['end'])
        self.assertEqual(phrase.cuts(self.document, self.lanes), [])


class HarmonicTests(unittest.TestCase):
    def test_real_spectra_find_the_weak_bar_and_a_better_source(self):
        rate, bpm = 16000, 120
        time = np.arange(rate * 2) / rate
        tone = lambda frequency: .1 * np.sin(2 * np.pi * frequency * time)
        good = tone(440)
        original = np.concatenate([good, good, tone(466.1638), good])
        alternative = np.tile(good, 4)
        lane_audio = np.concatenate([original, alternative])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, audio in [('original.wav', original), ('better.wav', alternative),
                                ('lane-01.wav', lane_audio),
                                ('premaster.wav', lane_audio + np.tile(good, 8))]:
                mixjam.write_wav(root / name, rate, audio[:, None])
            document = dict(song=dict(bpm=bpm), samples=[
                dict(id=1, name='original', path=str(root / 'original.wav'), sourceBpm=bpm),
                dict(id=2, name='better', path=str(root / 'better.wav'), sourceBpm=bpm)],
                lanes=[dict(placements=[dict(sampleId=sample, startTick=start,
                    durationTicks=1024, offsetFrames=0) for sample, start in [(1, 0), (2, 1024)]])])
            mix = clash.Mix(root, bpm)
            fits = mix.lane(1)
            _, low = clash.weak(fits)
            self.assertEqual([bar for bar, _ in low], [3])
            candidates = clash.alternatives(document, 1, 3, fits[3][1], clash.Sources(document))
            self.assertEqual(candidates[0][1], 'better')
            own_fit = next(score for score, _, _, own in candidates if own)
            self.assertGreaterEqual(candidates[0][0], own_fit + clash.BETTER)
            # The renderer writes one file for a lane and its cross-fade twin.
            document['lanes'][0]['placements'][0]['durationTicks'] = 512
            document['lanes'].append(dict(placements=[dict(sampleId=1, startTick=512,
                durationTicks=512, offsetFrames=rate * 4), document['lanes'][0]['placements'].pop()]))
            metadata = {'1': dict(name='Lead', role='lead'),
                        '2': dict(name='Lead (twin)', role='lead')}
            self.assertEqual(clash.tonal_tracks(document, metadata), [1])
            # Atmospheres are checked; field recordings are not.
            for role, field, expected in (('atmos', False, [1]), ('atmos', True, []), ('fx', False, [])):
                other = {'1': dict(name='Air', role=role, field=field), '2': dict(name='Air (twin)', role=role)}
                self.assertEqual(clash.tonal_tracks(document, other), expected)
            candidates = clash.alternatives(document, 1, 3, fits[3][1], clash.Sources(document, metadata))
            self.assertEqual(candidates[0][1], 'better')
            lines = []
            clash.report(document, metadata, root, lines.append)
            self.assertNotIn('unavailable', '\n'.join(lines))
            song = root / 'song.mixjam'
            song.write_text(json.dumps(document), encoding='utf-8')
            song.with_suffix('.meta.json').write_text(json.dumps({'lanes': metadata}), encoding='utf-8')
            output = io.StringIO()
            with mock.patch.object(sys, 'argv', ['clash.py', str(song), str(root), '--lanes', '2', '--alt']), \
                    mock.patch.object(sys, 'stdout', output):
                clash.main()
            self.assertIn('track 1 Lead: usual fit', output.getvalue())
            self.assertNotIn('missing', output.getvalue())

    def test_missing_lane_file_is_unavailable(self):
        with tempfile.TemporaryDirectory() as directory:
            with mock.patch.object(clash, 'mono', return_value=(44100, np.zeros(44100))):
                mix = clash.Mix(directory, 120)
                self.assertIsNone(mix.lane(1))
            self.assertIn('lane-01.wav is missing', mix.unavailable[1])

    def test_report_keeps_unavailable_catalog_sources_as_evidence(self):
        document = dict(song=dict(bpm=120), samples=[], lanes=[dict(placements=[dict(
            sampleId=42, startTick=0, durationTicks=1024, offsetFrames=0)])])
        fits = {1: (.9, np.ones(12), np.ones(12)), 2: (.9, np.ones(12), np.ones(12)),
                3: (.4, np.ones(12), np.ones(12))}
        lines = []
        with mock.patch.object(Path, 'glob', return_value=[Path('lane-01.wav')]), \
                mock.patch.object(clash, 'Mix') as mix:
            mix.return_value.lane.return_value = fits
            mix.return_value.unavailable = {}
            clash.report(document, {'1': dict(role='lead')}, '.', lines.append)
        self.assertIn('unavailable', '\n'.join(lines).lower())
        self.assertIn('42', '\n'.join(lines))

    def test_missing_source_does_not_abort_alternatives(self):
        sources = clash.Sources(dict(song=dict(bpm=120), lanes=[], samples=[dict(
            id=1, path='no-such-source.wav', name='missing')]))
        self.assertIsNone(sources.get(1))
        self.assertTrue(sources.unavailable)

    def test_alternatives_do_not_infer_positions_through_reverse_or_pitch(self):
        for extra in ({'reverse': True}, {'automation': {'pitch': [dict(tick=0, value=1200)]}}):
            with self.subTest(extra=extra):
                clip = dict(sampleId=1, startTick=0, durationTicks=1024, offsetFrames=0)
                lane = dict(placements=[clip])
                if 'reverse' in extra:
                    clip.update(extra)
                else:
                    lane.update(extra)
                document = dict(song=dict(bpm=120), samples=[], lanes=[lane])
                sources = clash.Sources(document)
                with mock.patch.object(sources, 'get', return_value=(44100, np.ones(4), 1, 0, 4)), \
                        mock.patch.object(sources, 'bar_chroma', return_value=np.ones(12)):
                    self.assertEqual(clash.alternatives(document, 1, 2, np.ones(12), sources), [])
                self.assertTrue(sources.unavailable)

    def test_lane_sample_rate_must_match_the_premaster(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'lane-01.wav').touch()
            with mock.patch.object(clash, 'mono', side_effect=[
                    (44100, np.zeros(44100)), (22050, np.zeros(44100))]):
                mix = clash.Mix(root, 120)
                self.assertIsNone(mix.lane(1))
            self.assertIn('sample rates differ', mix.unavailable[1])


if __name__ == '__main__':
    unittest.main()

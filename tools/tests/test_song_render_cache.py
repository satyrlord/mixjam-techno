"""Checks for the song tools' renders: keyed lanes, gain fits, and the lane cache."""
import json
import io
import math
import os
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from unittest import mock

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import mixjam  # noqa: E402
import master  # noqa: E402
import render  # noqa: E402
import samples  # noqa: E402
from audible import lane_bars  # noqa: E402


class RenderCacheTests(unittest.TestCase):
    def test_isolated_repeat_keeps_song_end_and_cache_duration(self):
        song = mixjam.Song('repeat duration')
        document = song.document()
        document['lanes'][0]['placements'] = [dict(id=1, sampleId=1_000_000, startTick=0, durationTicks=64)]
        document['lanes'][0]['automation'] = {'repeat': [dict(tick=0, value=16)]}
        document['lanes'][1]['placements'] = [dict(id=2, sampleId=1_000_000, startTick=192, durationTicks=64)]
        document['samples'] = [dict(id=1_000_000, name='tone', path='tone.wav', sourceBpm=140)]
        original = json.dumps(document)
        part = render.solo(document, [1])
        self.assertEqual(part['song']['loopRegion'], [0, 256])
        self.assertFalse(part['lanes'][1]['placements'])
        self.assertEqual(json.dumps(document), original)
        with mock.patch.object(render, 'exe_stamp', return_value='renderer'):
            before = render.lane_key(part, [1], False, {1_000_000: 'tone'})
            document['lanes'][1]['placements'][0]['startTick'] = 448
            after = render.lane_key(render.solo(document, [1]), [1], False, {1_000_000: 'tone'})
            self.assertNotEqual(before, after)
        document['song']['loopRegion'] = [128, 768]
        self.assertEqual(render.solo(document, [1])['song']['loopRegion'], [128, 768])
        document['song']['loopRegion'] = [128, 256]
        self.assertEqual(render.solo(document, [1])['song']['loopRegion'], [128, 512])

    def test_audibility_includes_repeat_runs_and_excludes_replaced_live_clips(self):
        document = mixjam.Song('repeat bars').document()
        lane = document['lanes'][0]
        lane['placements'] = [dict(startTick=0, durationTicks=64),
                              dict(startTick=768, durationTicks=64)]
        document['lanes'][1]['placements'] = [dict(startTick=1024, durationTicks=256)]
        lane['automation'] = {'repeat': [dict(tick=0, value=16), dict(tick=512, value=16),
                                         dict(tick=768, value=0)]}
        self.assertEqual(lane_bars(document, [1]), {0, 1, 2, 3})
        # A capture in silence replaces the clip at bar 4 with silence too.
        lane['automation']['repeat'] = [dict(tick=256, value=16)]
        self.assertEqual(lane_bars(document, [1]), {0})
        # Off before the first point, then two captures, with a new division.
        lane['automation']['repeat'] = [dict(tick=32, value=16), dict(tick=256, value=0),
                                         dict(tick=768, value=16), dict(tick=896, value=32)]
        self.assertEqual(lane_bars(document, [1]), {0, 1, 3, 4})
        # Adjacent off/on points take a new capture; the run ends at song end.
        lane['automation']['repeat'] = [dict(tick=0, value=16), dict(tick=256, value=0),
                                         dict(tick=257, value=16), dict(tick=1536, value=0)]
        self.assertEqual(lane_bars(document, [1]), {0, 1})

    def test_audibility_keeps_repeat_crossfades_at_bar_boundaries(self):
        document = mixjam.Song('repeat tails').document()
        lane = document['lanes'][0]
        lane['placements'] = [dict(startTick=0, durationTicks=64)]
        document['lanes'][1]['placements'] = [dict(startTick=512, durationTicks=256)]
        lane['automation'] = {'repeat': [dict(tick=0, value=16), dict(tick=256, value=0)]}
        self.assertEqual(lane_bars(document, [1]), {0, 1})
        # Only the outgoing crossfade reads this clip, just beyond the slice.
        lane['placements'] = [dict(startTick=16, durationTicks=1)]
        lane['automation']['repeat'] = [dict(tick=0, value=16), dict(tick=512, value=0)]
        self.assertEqual(lane_bars(document, [1]), {0, 1, 2})

    def test_keyed_lane_extracts_parts_without_clipping_the_key_into_its_output(self):
        song = mixjam.Song('keyed')
        song.ducker(2, 1)
        document = song.document()
        for index in (0, 1):
            document['lanes'][index]['placements'] = [dict(id=index + 1, startTick=0, durationTicks=64)]
        document['lanes'][0]['sends'] = [0.5] * 4
        document['lanes'][0]['automation'] = {'volume': [{'tick': 0, 'value': -3}],
                                             'sends': [[{'tick': 0, 'value': 0.5}]] * 4}
        def native(command, **kwargs):
            part = json.loads(Path(command[command.index('--mix') + 1]).read_text())
            if '--stems' in command:
                folder = Path(command[command.index('--stems') + 1])
                folder.mkdir(exist_ok=True)
                self.assertTrue(part['lanes'][0]['placements'])
                self.assertEqual(part['lanes'][0]['sends'], [0] * 4)
                self.assertEqual(part['lanes'][0]['automation'], {'volume': [{'tick': 0, 'value': -3}]})
                title = part['song']['title']
                mixjam.write_wav(folder / f'{title} - Track 01.wav', 44100, np.ones((8, 2)))
                mixjam.write_wav(folder / f'{title} - Track 02.wav', 44100, np.full((8, 2), 0.5))
                mixjam.write_wav(folder / f'{title} - FX 1 return.wav', 44100, np.full((8, 2), 0.125))
            else:
                key = -1 if part['lanes'][0]['polarityInverted'] else 1
                mixjam.write_wav(command[command.index('--output') + 1], 44100,
                                 np.full((8, 2), 0.5 + key))
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render.subprocess, 'run', side_effect=native):
            result = render.render_lanes(document, {'bass': [2]}, Path(directory), jobs=1)
            _, data = mixjam.read_wav(result['bass'])
            np.testing.assert_allclose(data, 0.625, atol=1 / 32768)

    def test_gain_fit_scales_unkeyed_lanes_with_the_premaster(self):
        document = mixjam.Song('fit lanes').document()
        document['lanes'][0]['placements'] = [dict(id=1)]
        document['lanes'][1]['placements'] = [dict(id=2)]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'song.mixjam'
            source.write_text(json.dumps(document))
            def lanes(current, groups, out, jobs, cache=None):
                paths = {}
                for name, level in (('track 1', 0.25), ('track 2', 0.125)):
                    paths[name] = out / f'{name}.wav'
                    mixjam.write_wav(paths[name], 44100, np.full((8, 2), level))
                return paths
            with mock.patch.object(render, 'render_lanes', side_effect=lanes), \
                    mock.patch('audible.audibility', return_value=[]), \
                    mock.patch.object(sys, 'argv', ['render.py', str(source), str(root), '--fit-gain', '-6', '--no-master']), \
                    mock.patch.object(sys, 'stdout', io.StringIO()):
                render.main()
            _, premaster = mixjam.read_wav(root / 'premaster.wav')
            _, one = mixjam.read_wav(root / 'track 1.wav')
            _, two = mixjam.read_wav(root / 'track 2.wav')
        np.testing.assert_allclose(premaster, 10 ** (-6 / 20), atol=2 / 32768)
        np.testing.assert_allclose(one + two, premaster, atol=2 / 32768)

    def test_gain_fit_rejects_clipped_parts_before_changing_song_or_audio(self):
        cases = [('lane', [0.75, -0.5], ['a', 'b']),
                 ('stem', [0.375, 0.375, -0.25, -0.25], ['a', 'a', 'b', 'b'])]
        for kind, levels, roles in cases:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                document = mixjam.Song('cancelling lanes').document()
                meta = {'lanes': {}}
                for track, role in enumerate(roles, 1):
                    document['lanes'][track - 1]['placements'] = [dict(id=track)]
                    meta['lanes'][str(track)] = dict(name=f'part {track}', role=role)
                source = root / 'song.mixjam'
                source.write_text(json.dumps(document), encoding='utf-8')
                original = source.read_bytes()
                source.with_suffix('.meta.json').write_text(json.dumps(meta), encoding='utf-8')
                paths, originals = {}, {}
                for track, level in enumerate(levels, 1):
                    path = root / f'lane-{track:02d}.wav'
                    mixjam.write_wav(path, 44100, np.full((8, 2), level))
                    paths[f'part {track}'] = path
                    originals[path] = path.read_bytes()
                for name in ('premaster.wav', 'stem-a.wav', 'audible.txt'):
                    path = root / name
                    path.write_bytes(b'previous output')
                    originals[path] = path.read_bytes()
                with mock.patch.object(render, 'render_lanes', return_value=paths), \
                        mock.patch('audible.audibility') as audibility, \
                        mock.patch.object(sys, 'argv', ['render.py', str(source), str(root),
                                                      '--fit-gain', '-6', '--no-master']), \
                        mock.patch.object(sys, 'stdout', io.StringIO()), \
                        self.assertRaisesRegex(SystemExit, f'{kind}.*exceed'):
                    render.main()
                self.assertEqual(source.read_bytes(), original)
                for path, data in originals.items():
                    self.assertEqual(path.read_bytes(), data, path.name)
                audibility.assert_not_called()

    def test_gain_fit_renders_keyed_audio_at_the_saved_gain(self):
        song = mixjam.Song('fit')
        song.ducker(2, 1)
        document = song.document()
        document['lanes'][1]['placements'] = [dict(id=1)]
        seen = []
        def lanes(current, groups, out, jobs, cache=None):
            gain = current['song']['masterGain']
            seen.append(gain)
            path = out / 'lane-02.wav'
            # A threshold-driven processor does not scale linearly with input gain.
            mixjam.write_wav(path, 44100, np.full((8, 2), 0.4 * math.sqrt(gain)))
            return {'track 2': path}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'song.mixjam'
            source.write_text(json.dumps(document))
            with mock.patch.object(render, 'render_lanes', side_effect=lanes), \
                    mock.patch('audible.audibility', return_value=[]), \
                    mock.patch.object(sys, 'argv', ['render.py', str(source), str(root), '--fit-gain', '-6', '--no-master']), \
                    mock.patch.object(sys, 'stdout', io.StringIO()):
                render.main()
            final_gain = json.loads(source.read_text())['song']['masterGain']
            _, pcm = mixjam.read_wav(root / 'premaster.wav')
            self.assertEqual(seen[-1], final_gain)
            self.assertGreater(len(seen), 1)
            np.testing.assert_allclose(pcm, 0.4 * math.sqrt(final_gain), atol=1 / 32768)

    def test_keyed_repeat_keeps_song_end_and_cache_duration(self):
        for repeated in (None, 1, 2):
            with self.subTest(repeated=repeated):
                song = mixjam.Song('keyed repeat duration')
                song.ducker(2, 1)
                document = song.document()
                for index, lane in enumerate(document['lanes'][:3]):
                    lane['placements'] = [dict(id=index + 1, sampleId=1_000_000,
                                               startTick=0, durationTicks=64 if index < 2 else 256)]
                document['samples'] = [dict(id=1_000_000, name='tone')]
                if repeated is not None:
                    document['lanes'][repeated - 1]['automation'] = {'repeat': [dict(tick=0, value=16)]}
                original = json.dumps(document)
                part = render.keyed_document(document, [2])
                self.assertEqual(part['song']['loopRegion'], [0, 256])
                self.assertEqual(json.dumps(document), original)
                with mock.patch.object(render, 'exe_stamp', return_value='renderer'):
                    before = render.lane_key(part, [2], True, {1_000_000: 'tone'})
                    document['lanes'][2]['placements'][0]['durationTicks'] = 512
                    after = render.lane_key(render.keyed_document(document, [2]), [2], True,
                                            {1_000_000: 'tone'})
                    self.assertNotEqual(before, after)
                document['song']['loopRegion'] = [128, 768]
                self.assertEqual(render.keyed_document(document, [2])['song']['loopRegion'], [128, 768])
                document['song']['loopRegion'] = [128, 256]
                self.assertEqual(render.keyed_document(document, [2])['song']['loopRegion'], [128, 512])

    def test_keyed_lane_plays_only_its_tracks_and_direct_keys(self):
        song = mixjam.Song('closure')
        song.ducker(2, 1)
        song.ducker(1, 3)
        document = song.document()
        for index in range(5):
            document['lanes'][index]['placements'] = [dict(id=index + 1, sampleId=1_000_000 + index,
                                                         startTick=0, durationTicks=64)]
            document['lanes'][index]['automation'] = {'volume': [{'tick': 0, 'value': -3}]}
            document['samples'].append(dict(id=1_000_000 + index, name=f'sound {index}'))
        part = render.keyed_document(document, [2])
        self.assertEqual([bool(lane['placements']) for lane in part['lanes'][:5]], [True, True, False, False, False])
        self.assertNotIn('automation', part['lanes'][3])
        # A change to a track outside the closure does not change the lane's cache key.
        with mock.patch.object(render, 'exe_stamp', return_value='renderer'):
            fingerprints = {1_000_000 + index: f'audio {index}' for index in range(5)}
            original = render.lane_key(part, [2], True, fingerprints)
            document['lanes'][4]['gain'] = 0.5
            self.assertEqual(render.lane_key(render.keyed_document(document, [2]), [2], True, fingerprints),
                             original)
            document['lanes'][2]['gain'] = 0.5
            self.assertEqual(render.lane_key(render.keyed_document(document, [2]), [2], True, fingerprints),
                             original)
            document['lanes'][0]['gain'] = 0.5
            self.assertNotEqual(render.lane_key(render.keyed_document(document, [2]), [2], True, fingerprints),
                                original)

    def test_fx_key_alone_uses_the_complete_key_context(self):
        song = mixjam.Song('fx key')
        document = song.document()
        document['fxBuses'][0]['module'] = {'type': 'echoform-delay', 'duckKey': 3}
        self.assertEqual(render.ducker_keys(document, [2]), {3})
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render, 'keyed_parts') as parts, \
                mock.patch.object(render, 'run') as run:
            render.render_lanes(document, {'pad': [2]}, Path(directory), jobs=1)
            parts.assert_called_once()
            run.assert_not_called()

    def test_lane_cache_renders_only_changed_lanes(self):
        document = mixjam.Song('cache').document()
        document['lanes'][0]['placements'] = [dict(id=1, sampleId=1_000_000, startTick=0, durationTicks=96)]
        document['lanes'][1]['placements'] = [dict(id=2, sampleId=1_000_001, startTick=0, durationTicks=96)]
        renders = []
        def native(command, **kwargs):
            part = json.loads(Path(command[command.index('--mix') + 1]).read_text())
            renders.append([index for index, lane in enumerate(part['lanes'], 1) if lane['placements']])
            mixjam.write_wav(command[command.index('--output') + 1], 44100,
                             np.full((8, 2), 0.25 * part['song']['masterGain']))
            return mock.Mock(returncode=0)
        groups = {'a': [1], 'b': [2]}
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render.subprocess, 'run', side_effect=native), \
                mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            root = Path(directory)
            for name in ('a', 'b'):
                mixjam.write_wav(root / f'{name}.wav', 44100, np.ones((8, 2)))
            document['samples'] = [dict(id=1_000_000 + index, name=name, path=str(root / f'{name}.wav'),
                                        sourceBpm=140) for index, name in enumerate(('a', 'b'))]
            (root / 'r1').mkdir()
            (root / 'r2').mkdir()
            render.render_lanes(document, groups, root / 'r1', jobs=1, cache=root / 'cache')
            self.assertEqual(sorted(renders), [[1], [2]])
            # A script that adds a sound renumbers the ids; only lane b's clip moves.
            changed = json.loads(json.dumps(document))
            for sample in changed['samples']:
                sample['id'] += 1
            changed['lanes'][0]['placements'] = [dict(id=5, sampleId=1_000_001, startTick=0, durationTicks=96)]
            changed['lanes'][1]['placements'] = [dict(id=6, sampleId=1_000_002, startTick=96, durationTicks=96)]
            renders.clear()
            result = render.render_lanes(changed, groups, root / 'r2', jobs=1, cache=root / 'cache')
            self.assertEqual(renders, [[2]])
            _, cached = mixjam.read_wav(result['a'])
            np.testing.assert_allclose(cached, 0.25 * document['song']['masterGain'], atol=1 / 32768)
            # The first round's lane stays as it was rendered.
            _, first = mixjam.read_wav(root / 'r1' / 'lane-01.wav')
            np.testing.assert_allclose(first, 0.25 * document['song']['masterGain'], atol=1 / 32768)

    def test_lane_cache_tracks_source_gain_and_global_solo_changes(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            root = Path(directory)
            source = root / 'source.wav'
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.5))
            song = mixjam.Song('cache dependencies')
            song.clip(1, source, 0, 96)
            document = song.document()
            calls = []

            def native(path, wav, data):
                part = json.loads(path.read_text())
                calls.append(part)
                rate, pcm = mixjam.read_wav(source)
                if any(lane['solo'] for lane in part['lanes']) and not part['lanes'][0]['solo']:
                    pcm *= 0
                mixjam.write_wav(wav, rate, pcm * part['song']['masterGain'])

            def rendered(name):
                out = root / name
                out.mkdir()
                with mock.patch.object(render, 'run', side_effect=native):
                    result = render.render_lanes(document, {'one': [1]}, out, 1, root / 'cache')
                return float(np.max(mixjam.read_wav(result['one'])[1]))

            self.assertEqual(rendered('first'), 0.5)
            self.assertEqual(rendered('hit'), 0.5)
            self.assertEqual(len(calls), 1)
            # Keep the file's size and timestamp: audio identity must still change.
            stamp = source.stat()
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.25))
            os.utime(source, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
            self.assertEqual(rendered('new-source'), 0.25)
            document['lanes'][1]['solo'] = True
            self.assertEqual(rendered('other-solo'), 0)
            document['lanes'][1]['solo'] = False
            document['song']['masterGain'] = 0
            self.assertEqual(rendered('zero'), 0)
            document['song']['masterGain'] = 4
            self.assertAlmostEqual(rendered('loud'), 1, delta=1 / 32768)
            document['song']['masterGain'] = 2
            self.assertEqual(rendered('quieter'), 0.5)
            self.assertEqual(len(calls), 6)

    def test_section_changes_reuse_audio_only_when_the_sections_are_valid(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            root = Path(directory)
            source = root / 'source.wav'
            mixjam.write_wav(source, 44100, np.ones((8, 2)))
            song = mixjam.Song('sections')
            song.clip(1, source, 0, 96)
            document = song.document()
            key = render.lane_key(document, [1], False)
            document['song']['sections'] = [dict(startTick=0, name='Intro')]
            self.assertEqual(render.lane_key(document, [1], False), key)
            document['song']['sections'][0]['name'] = 'Renamed'
            with mock.patch.object(render, 'from_cache', return_value=True) as cached, \
                    mock.patch.object(render, 'run') as native:
                render.render_lanes(document, {'one': [1]}, root, 1, root / 'cache')
                cached.assert_called_once()
                native.assert_not_called()
            variants = [(18, []), (18, [dict(startTick=0, name='Intro')]),
                        (19, None), (19, {}), (19, [dict(startTick=0)]),
                        (19, [dict(startTick=0, name='Intro', extra=True)])]
            for tick in (-1, 4096 * 256, True, 0.0):
                variants.append((19, [dict(startTick=tick, name='Intro')]))
            for name in ('', ' Intro', 'Intro ', 'a\nb', '\x7f', '\x85', '\ud800', 'x' * 49, None):
                variants.append((19, [dict(startTick=0, name=name)]))
            variants.append((19, [dict(startTick=256, name='A'), dict(startTick=0, name='B')]))
            variants.append((19, [dict(startTick=0, name='A'), dict(startTick=0, name='B')]))
            for version, sections in variants:
                with self.subTest(version=version, sections=sections):
                    document['formatVersion'] = version
                    document['song']['sections'] = sections
                    with mock.patch.object(render, 'from_cache', return_value=True) as cached, \
                            mock.patch.object(render, 'run', side_effect=SystemExit('native validation')) as native:
                        with self.assertRaisesRegex(SystemExit, 'native validation'):
                            render.render_lanes(document, {'one': [1]}, root, 1, root / 'cache')
                        cached.assert_not_called()
                        native.assert_called_once()

    def test_cache_bypasses_sources_that_need_native_resolution(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            source = Path(directory) / 'source.wav'
            mixjam.write_wav(source, 44100, np.ones((8, 2)))
            song = mixjam.Song('source dependencies')
            song.clip(1, source, 0, 96)
            document = song.document()
            self.assertIsNotNone(render.lane_key(document, [1], False))
            variants = []
            for field, value in [('formatVersion', 17), ('libraries', [{'product': 't2', 'root': 'library'}]),
                                 ('missingSamples', [{'id': 1}]), ('sampleConflicts', [{'id': 1}])]:
                changed = render.copy(document)
                changed[field] = value
                variants.append(changed)
            for sample_id in (1, 2, 1_000_005):
                changed = render.copy(document)
                changed['lanes'][0]['placements'][0]['sampleId'] = sample_id
                variants.append(changed)
            for field, value in [('path', 'relative.wav'), ('path', str(source.with_name('missing.wav'))),
                                 ('sourceBpm', None), ('origin', 't2')]:
                changed = render.copy(document)
                changed['samples'][0][field] = value
                variants.append(changed)
            for changed in variants:
                with self.subTest(document=changed):
                    self.assertIsNone(render.lane_key(changed, [1], False))

    def test_cache_does_not_publish_audio_when_its_source_changes_during_render(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            root = Path(directory)
            source = root / 'source.wav'
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.25))
            song = mixjam.Song('changing source')
            song.clip(1, source, 0, 96)
            document = song.document()
            calls = []

            def native(path, wav, data):
                calls.append(path)
                rate, pcm = mixjam.read_wav(source)
                mixjam.write_wav(wav, rate, pcm)
                mixjam.write_wav(source, rate, np.full((8, 2), 0.5))

            with mock.patch.object(render, 'run', side_effect=native):
                render.render_lanes(document, {'one': [1]}, root, 1, root / 'cache')
                self.assertEqual(list((root / 'cache').rglob('*.wav')), [])
                result = render.render_lanes(document, {'one': [1]}, root, 1, root / 'cache')
            self.assertEqual(len(calls), 2)
            np.testing.assert_array_equal(mixjam.read_wav(result['one'])[1], 0.5)

    def test_cache_key_keeps_source_provenance(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            source = Path(directory) / 'source.wav'
            mixjam.write_wav(source, 44100, np.ones((8, 2)))
            song = mixjam.Song('provenance')
            song.clip(1, source, 0, 96)
            document = song.document()
            original = render.lane_key(document, [1], False)
            document['sourceNotices'] = ['adapter provenance']
            self.assertNotEqual(render.lane_key(document, [1], False), original)

    def test_cache_shares_storage_and_damaged_entries_are_misses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cache, source, output = root / 'cache', root / 'source.wav', root / 'output.wav'
            key = 'a' * 32
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.25))
            render.to_cache(cache, key, source)
            entry = cache / render.CACHE_VERSION / f'{key}.wav'
            self.assertTrue(os.path.samefile(entry, source))
            # A tool that rewrites a delivered lane replaces it, so the cache keeps its audio.
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.5))
            self.assertTrue(render.from_cache(cache, key, output))
            self.assertTrue(os.path.samefile(entry, output))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.25)
            mixjam.write_wav(output, 44100, np.zeros((8, 2)))
            self.assertTrue(render.from_cache(cache, key, output))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.25)
            # A program that writes into the shared file changes the entry: it is a miss.
            audio = output.read_bytes()
            with output.open('r+b') as shared:
                shared.seek(len(audio) - 4)
                shared.write(b'\x01\x02\x03\x04')
            other = root / 'other.wav'
            self.assertFalse(render.from_cache(cache, key, other))
            self.assertFalse(other.exists())
            entry.write_bytes(audio[:-4])
            self.assertFalse(render.from_cache(cache, key, other))
            entry.write_bytes(b'bad WAV')
            self.assertFalse(render.from_cache(cache, key, other))
            entry.unlink()
            self.assertFalse(render.from_cache(cache, key, other))

    def test_cache_requires_a_digest_and_preserves_the_output_on_a_miss(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cache, output = root / 'cache', root / 'output.wav'
            key = 'a' * 32
            folder = cache / render.CACHE_VERSION
            folder.mkdir(parents=True)
            mixjam.write_wav(folder / f'{key}.wav', 44100, np.full((8, 2), 0.25))
            mixjam.write_wav(output, 44100, np.full((8, 2), 0.5))
            self.assertFalse(render.from_cache(cache, key, output))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.5)

    def test_cache_rejects_a_first_publication_without_its_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cache, source = root / 'cache', root / 'source.wav'
            key = 'a' * 32
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.25))
            replace = os.replace

            def fail_digest(source, destination):
                if Path(destination).suffix == '.sha256':
                    raise OSError('digest publication failed')
                return replace(source, destination)

            with mock.patch.object(render.os, 'replace', side_effect=fail_digest):
                render.to_cache(cache, key, source)
            self.assertFalse(render.from_cache(cache, key, root / 'output.wav'))
            self.assertEqual(list((cache / render.CACHE_VERSION).glob('*.tmp')), [])

    def test_cache_validates_the_staged_file_before_replacing_the_output(self):
        for fallback in (False, True):
            for truncated in (False, True):
                with self.subTest(fallback=fallback, truncated=truncated), \
                        tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    cache, source, output = root / 'cache', root / 'source.wav', root / 'output.wav'
                    key = 'a' * 32
                    mixjam.write_wav(source, 44100, np.full((8, 2), 0.25))
                    render.to_cache(cache, key, source)
                    mixjam.write_wav(output, 44100, np.full((8, 2), 0.125))
                    replacement = root / 'replacement.wav'
                    mixjam.write_wav(replacement, 44100, np.full((8, 2), 0.5))
                    if truncated:
                        replacement.write_bytes(replacement.read_bytes()[:-4])
                    link = os.link

                    def replace_before_link(source, destination):
                        os.replace(replacement, source)
                        if fallback:
                            raise OSError('hard links unavailable')
                        return link(source, destination)

                    with mock.patch.object(render.os, 'link', side_effect=replace_before_link):
                        self.assertFalse(render.from_cache(cache, key, output))
                    np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.125)
                    self.assertEqual(list(root.glob('*.tmp')), [])

    def test_cache_uses_independent_copies_when_hard_links_are_unavailable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cache, source, output = root / 'cache', root / 'source.wav', root / 'output.wav'
            key = 'a' * 32
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.25))
            with mock.patch.object(render.os, 'link', side_effect=OSError('hard links unavailable')):
                render.to_cache(cache, key, source)
                self.assertTrue(render.from_cache(cache, key, output))
            entry = cache / render.CACHE_VERSION / f'{key}.wav'
            self.assertFalse(os.path.samefile(entry, source))
            self.assertFalse(os.path.samefile(entry, output))
            output.write_bytes(b'edited output')
            self.assertTrue(render.from_cache(cache, key, output))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.25)

    def test_failed_wav_replacement_preserves_existing_audio_and_cleans_the_stage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output, alias = root / 'output.wav', root / 'alias.wav'
            mixjam.write_wav(output, 44100, np.full((8, 2), 0.25))
            os.link(output, alias)
            with mock.patch.object(mixjam.os, 'replace', side_effect=OSError('replacement failed')):
                with self.assertRaisesRegex(OSError, 'replacement failed'):
                    mixjam.write_wav(output, 44100, np.zeros((8, 2)))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.25)
            self.assertTrue(os.path.samefile(output, alias))
            self.assertEqual(list(root.glob('*.tmp')), [])

    def test_cache_pruning_removes_the_digest_of_a_removed_entry(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            source = cache / 'source.wav'
            mixjam.write_wav(source, 44100, np.zeros((8, 2)))
            old, new = 'a' * 32, 'b' * 32
            render.to_cache(cache, old, source)
            render.to_cache(cache, new, source)
            folder = cache / render.CACHE_VERSION
            os.utime(folder / f'{old}.wav', (1, 1))
            os.utime(folder / f'{new}.wav')
            render.prune_cache(cache, limit=source.stat().st_size)
            self.assertFalse((folder / f'{old}.sha256').exists())
            self.assertTrue((folder / f'{new}.sha256').exists())

    def test_cache_publication_failure_preserves_existing_entry(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, cache = root / 'source.wav', root / 'cache'
            key = 'a' * 32
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.25))
            render.to_cache(cache, key, source)
            mixjam.write_wav(source, 44100, np.zeros((8, 2)))
            with mock.patch.object(render.os, 'replace', side_effect=OSError('interrupted')):
                render.to_cache(cache, key, source)
            output = root / 'output.wav'
            self.assertTrue(render.from_cache(cache, key, output))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.25)
            self.assertEqual(list((cache / render.CACHE_VERSION).glob('*.tmp')), [])

    def test_cache_pruning_keeps_unrelated_wavs_and_the_newest_entry(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            unrelated = cache / 'original.wav'
            mixjam.write_wav(unrelated, 44100, np.zeros((8, 2)))
            old, new = 'a' * 32, 'b' * 32
            render.to_cache(cache, old, unrelated)
            render.to_cache(cache, new, unrelated)
            folder = cache / render.CACHE_VERSION
            extra = folder / 'original.wav'
            mixjam.write_wav(extra, 44100, np.zeros((8, 2)))
            os.utime(folder / f'{old}.wav', (1, 1))
            render.prune_cache(cache, limit=unrelated.stat().st_size)
            self.assertTrue(unrelated.exists())
            self.assertTrue(extra.exists())
            self.assertFalse((folder / f'{old}.wav').exists())
            self.assertTrue((folder / f'{new}.wav').exists())

    def test_silent_keyed_song_uses_the_native_silent_mix(self):
        song = mixjam.Song('silent')
        song.ducker(2, 1)
        def native(command, **kwargs):
            if '--stems' in command:
                return mock.Mock(returncode=1, stdout='',
                                 stderr='No track sounds: add clips, or check Mute, Solo, and L/R.\n')
            mixjam.write_wav(command[command.index('--output') + 1], 44100, np.zeros((8, 2)))
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render.subprocess, 'run', side_effect=native):
            result = render.render_lanes(song.document(), {'bass': [2]}, Path(directory), jobs=1)
            _, pcm = mixjam.read_wav(result['bass'])
            np.testing.assert_array_equal(pcm, 0)

    def test_renders_yield_the_processor(self):
        calls = []
        def native(command, **kwargs):
            calls.append(kwargs)
            return mock.Mock(returncode=0)
        with mock.patch.object(render.subprocess, 'run', side_effect=native), \
                mock.patch.object(render, 'contain') as contain:
            render.run('song.mixjam', 'out.wav', Path('data'))
        contain.assert_called_once()
        if sys.platform == 'win32':
            self.assertEqual(calls[0]['creationflags'], render.BELOW_NORMAL_PRIORITY_CLASS)
        else:
            self.assertNotIn('creationflags', calls[0])
        pools, executor = [], render.ThreadPoolExecutor
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(render.os, 'cpu_count', return_value=32), \
                mock.patch.object(render, 'ThreadPoolExecutor',
                                  side_effect=lambda jobs: pools.append(jobs) or executor(jobs)):
            render.render_lanes(mixjam.Song('jobs').document(), {}, Path(directory))
        self.assertEqual(pools, [render.MAX_JOBS])

    def test_posix_lanes_and_master_inherit_lower_priority(self):
        priority, calls = threading.local(), {}
        together = threading.Barrier(2)
        caller, executor = threading.current_thread(), render.ThreadPoolExecutor
        def nice(increment):
            priority.value = getattr(priority, 'value', 0) + increment
            return priority.value
        def pool(jobs):
            # Linux stores niceness per thread. New workers inherit their creator's value.
            inherited = getattr(priority, 'value', 0)
            return executor(jobs, initializer=lambda: setattr(priority, 'value', inherited))
        def native(command, **kwargs):
            calls[Path(command[command.index('--output') + 1]).name] = getattr(priority, 'value', 0)
            if threading.current_thread() is not caller:
                together.wait(timeout=5)
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(render.sys, 'platform', 'linux'), \
                mock.patch.object(render.os, 'nice', side_effect=nice, create=True), \
                mock.patch.object(render, '_contained', False), \
                mock.patch.object(render.env, 'exe', return_value=Path('renderer')), \
                mock.patch.object(render, 'ThreadPoolExecutor', side_effect=pool), \
                mock.patch.object(render.subprocess, 'run', side_effect=native):
            root = Path(directory)
            render.render_lanes(mixjam.Song('priority').document(), {'one': [1], 'two': [2]}, root, jobs=2)
            render.run(root / 'song.mixjam', root / 'master.wav')
        self.assertEqual(calls, {'lane-01.wav': render.POSIX_NICENESS,
                                 'lane-02.wav': render.POSIX_NICENESS,
                                 'master.wav': render.POSIX_NICENESS})


if __name__ == '__main__':
    unittest.main()

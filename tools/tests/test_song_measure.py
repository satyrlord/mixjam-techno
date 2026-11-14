"""Checks for the song tools' measures, master fit, harmony, and sample index."""
import json
import io
import math
import os
from pathlib import Path
import sys
import struct
import tempfile
import unittest
import wave
from unittest import mock

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import env  # noqa: E402
import harmony  # noqa: E402
import mixjam  # noqa: E402
import master  # noqa: E402
import measure  # noqa: E402
import render  # noqa: E402
import midinotes  # noqa: E402
import samples  # noqa: E402
from audible import heard_share  # noqa: E402
from fit import fits  # noqa: E402
from master import album_profile, flag, misses, tonal_indices, THIRDS  # noqa: E402
from midichords import shift_for  # noqa: E402


class MeasureTests(unittest.TestCase):
    def short_song_report(self, frames=18900, sections=()):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mono = (np.sin(np.arange(frames) * 2 * math.pi * 440 / 44100) * 3000).astype('<i2')
            stereo = np.repeat(mono[:, None], 2, axis=1).tobytes()
            for name in ('source.wav', 'master.wav', 'premaster.wav'):
                with wave.open(str(root / name), 'wb') as output:
                    output.setparams((2, 2, 44100, frames, 'NONE', 'not compressed'))
                    output.writeframes(stereo)
            song = mixjam.Song('short')
            song.lane(1, 'drums', 'drums')
            song.clip(1, root / 'source.wav', 0, round(mixjam.ticks_of(frames, 44100, 140)))
            song.sections = list(sections)
            path = song.save(root / 'short.mixjam')
            captured = io.StringIO()
            with mock.patch.object(sys, 'argv', ['measure.py', str(path), str(root)]), \
                    mock.patch.object(sys, 'stdout', captured), \
                    mock.patch.object(env, 'reference_profile', return_value=None):
                measure.main()
            return captured.getvalue()

    def test_sub_bar_song_without_sections_reports_its_audio(self):
        report = self.short_song_report()
        self.assertIn('| song | 1-1 |', report)
        self.assertIn('Sections at least 4 dB below the loudest: 0 of 1.', report)

    def test_sections_outside_the_render_report_unavailable_measurements(self):
        report = self.short_song_report(sections=[dict(name='later', first_bar=10, bars=4)])
        self.assertIn('Section measurements unavailable', report)
        self.assertNotIn('Sections at least 4 dB below the loudest:', report)
        self.assertIn('## Arrangement', report)

    def test_audio_below_the_measurement_window_reports_unavailable_measurements(self):
        report = self.short_song_report(frames=1000)
        self.assertIn('Master tone measurements unavailable', report)
        self.assertIn('Section measurements unavailable', report)
        self.assertIn('## Arrangement', report)

    def test_master_tempo_precedence_and_missing_sections(self):
        rate = 8000
        wave_data = 0.2 * np.sin(np.arange(rate * 26) * 2 * math.pi * 440 / rate)
        audio = np.repeat(wave_data[:, None], 2, axis=1)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            song = root / 'song.mixjam'
            song.write_text(json.dumps(mixjam.Song('tempo', bpm=160).document()), encoding='utf-8')
            metadata = song.with_suffix('.meta.json')
            board = root / 'board.json'
            board.write_text(json.dumps(mixjam.board()), encoding='utf-8')
            premaster = root / 'renders' / 'premaster.wav'
            sections = [dict(name='intro', first_bar=1, bars=2)]
            cases = [
                ('song-no-meta', ['--song', str(song)], None, 160),
                ('song-over-meta', ['--song', str(song)], {'bpm': 130, 'sections': sections}, 160),
                ('song-partial-meta', ['--song', str(song)], {'bpm': 130}, 160),
                ('explicit-over-song', ['--song', str(song), '--bpm', '125'],
                 {'bpm': 130, 'sections': sections}, 125),
                ('metadata-no-song', [], {'bpm': 130, 'sections': sections}, 130),
                ('partial-meta-no-song', [], {'bpm': 130}, 130),
                ('no-source', [], None, 140),
            ]
            for name, options, meta, expected in cases:
                with self.subTest(name=name):
                    metadata.unlink(missing_ok=True)
                    if meta is not None:
                        metadata.write_text(json.dumps(meta), encoding='utf-8')
                    out = root / name
                    out.mkdir()
                    (out / 'master.wav').touch()
                    argv = ['master.py', str(premaster), str(board), str(out), '--no-render', *options]
                    captured = io.StringIO()
                    with mock.patch.object(sys, 'argv', argv), mock.patch.object(sys, 'stdout', captured), \
                            mock.patch.object(master, 'read_wav', return_value=(rate, audio)), \
                            mock.patch.object(master, 'load_profile', return_value=(None, None)), \
                            mock.patch.object(master, 'measure', wraps=master.measure) as measurements, \
                            mock.patch.object(master, 'drums', wraps=master.drums) as drum_measurement:
                        master.main()
                    self.assertTrue(all(call.args[2] == expected for call in measurements.call_args_list))
                    self.assertEqual(drum_measurement.call_args.args[4], expected)
                    if meta is None:
                        self.assertIn('No section metadata is available.', captured.getvalue())
                    elif 'sections' not in meta:
                        self.assertIn('The metadata has no measurable sections.', captured.getvalue())
                    if name == 'song-no-meta':
                        result = json.loads((out / 'measure.json').read_text(encoding='utf-8'))
                        self.assertEqual(len(result['premaster']['block_rms']), 2)

    def test_master_fit_uses_the_song_tempo_without_metadata(self):
        rate = 8000
        wave_data = 0.2 * np.sin(np.arange(rate * 26) * 2 * math.pi * 440 / rate)
        audio = np.repeat(wave_data[:, None], 2, axis=1)
        target = master.measure(audio, rate, 160)['loud_rms']
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            song = root / 'song.mixjam'
            song.write_text(json.dumps(mixjam.Song('tempo', bpm=160).document()), encoding='utf-8')
            board = root / 'board.json'
            board.write_text(json.dumps(mixjam.board()), encoding='utf-8')
            out = root / 'out'
            argv = ['master.py', str(root / 'premaster.wav'), str(board), str(out),
                    '--song', str(song), '--fit', str(target), '--quiet']
            with mock.patch.object(sys, 'argv', argv), mock.patch.object(sys, 'stdout', io.StringIO()), \
                    mock.patch.object(master, 'read_wav', return_value=(rate, audio)), \
                    mock.patch.object(master, 'load_profile', return_value=(None, None)), \
                    mock.patch.object(master, 'render_board', return_value=out / 'master.wav') as render_board:
                master.main()
            self.assertEqual(render_board.call_args.args[3], 160)
            result = json.loads((out / 'measure.json').read_text(encoding='utf-8'))
            self.assertEqual(len(result['premaster']['block_rms']), 2)
            self.assertEqual(len(result['master']['block_rms']), 2)

    def test_master_rejects_invalid_supplied_songs_even_with_bpm_override(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            song = root / 'song.mixjam'
            board = root / 'board.json'
            board.write_text(json.dumps(mixjam.board()), encoding='utf-8')
            cases = [None, '{', '{}', '{"song": {}}', '{"song": {"bpm": "160"}}',
                     '{"song": {"bpm": 0}}', '{"song": {"bpm": 128}}',
                     '{"song": {"bpm": NaN}}', '{"song": {"bpm": true}}']
            for content in cases:
                with self.subTest(content=content):
                    song.unlink(missing_ok=True)
                    if content is not None:
                        song.write_text(content, encoding='utf-8')
                    out = root / 'out'
                    argv = ['master.py', str(root / 'premaster.wav'), str(board), str(out),
                            '--song', str(song), '--bpm', '125']
                    with mock.patch.object(sys, 'argv', argv), self.assertRaises(SystemExit):
                        master.main()
                    self.assertFalse(out.exists())

    def test_exhausted_master_fit_returns_the_last_rendered_board(self):
        gains = []
        def record_render(premaster, board, out, bpm):
            gains.append(board['limiter']['gain'])
            return 'unused.wav'
        readings = [-10.5, -10.4, -10.3, -10.2, -10.4]
        with mock.patch.object(master, 'render_board', side_effect=record_render), \
                mock.patch.object(master, 'read_wav', return_value=(44100, np.ones((8, 2)))), \
                mock.patch.object(master, 'measure', side_effect=[{'loud_rms': x} for x in readings]):
            board, result = master.fit('unused.wav', mixjam.board(limiter={'gain': 4}),
                                       Path('.'), -10, 140, None)
        self.assertEqual(len(gains), 5)
        self.assertEqual(board['limiter']['gain'], gains[-1])
        self.assertEqual(result['loud_rms'], readings[-1])

    def test_gain_fit_does_not_replace_a_song_with_an_invalid_gain(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            song = root / 'song.mixjam'
            original = json.dumps(mixjam.Song('quiet').document())
            song.write_text(original, encoding='utf-8')
            with self.assertRaisesRegex(SystemExit, 'gain'):
                render.fit_gain(song, 0.01, -0.5)
            self.assertEqual(song.read_text(encoding='utf-8'), original)
            # A -6 dBFS premaster needs 5.5 dB of gain to reach -0.5 dBFS.
            self.assertAlmostEqual(render.fit_gain(song, 10 ** (-6 / 20), -0.5), 10 ** (5.5 / 20), places=5)

    def test_lanes_cover_every_used_track_and_sum_to_the_premaster_and_stems(self):
        document = mixjam.Song('lanes').document()
        clip = dict(id=1, sampleId=1, startTick=0, durationTicks=256, offsetFrames=0, gain=1)
        for track in (1, 2, 5):
            document['lanes'][track - 1]['placements'] = [dict(clip)]
        meta = {'lanes': {'1': {'name': 'Pad', 'role': 'pad'}, '2': {'name': 'Pad (twin)', 'role': 'pad'}}}
        groups = render.lane_groups(document, meta)
        self.assertEqual(groups, {'Pad': [1, 2], 'track 5': [5]})
        self.assertEqual(render.lane_roles(groups, meta), {'Pad': 'pad'})
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mixjam.write_wav(root / 'pad.wav', 44100, np.full((4, 2), 0.25))
            mixjam.write_wav(root / 'other.wav', 44100, np.full((6, 2), 0.5))
            rate, premaster, stems = render.mix_down({'Pad': root / 'pad.wav', 'track 5': root / 'other.wav'},
                                                     {'Pad': 'pad'})
        self.assertEqual(rate, 44100)
        self.assertEqual(premaster.shape, (6, 2))
        np.testing.assert_allclose(premaster[:, 0], [0.75] * 4 + [0.5] * 2, atol=1e-4)
        np.testing.assert_allclose(stems['pad'][:, 1], [0.25] * 4, atol=1e-4)

    def test_catalog_and_missing_samples_have_arrangement_labels(self):
        song = mixjam.Song('catalog').document()
        song['lanes'][0]['placements'] = [dict(id=1, sampleId=2, startTick=0,
                                               durationTicks=256, offsetFrames=0, gain=1)]
        song['lanes'][1]['placements'] = [dict(id=2, sampleId=43, startTick=0,
                                               durationTicks=256, offsetFrames=0, gain=1)]
        song['missingSamples'] = [dict(id=43, name='missing loop')]
        meta = measure.default_meta(song)
        lines = []
        measure.arrangement(song, meta['lanes'], meta['sections'], 1, lines.append)
        self.assertTrue(any('2 sounds' in line for line in lines))
        problems = measure.mechanical(song, meta['lanes'])
        self.assertTrue(any('missing loop' in problem for problem in problems))
        self.assertTrue(any('#2' in problem and 'unavailable' in problem for problem in problems))
        song['lanes'][0]['placements'].append(dict(id=3, sampleId=43, startTick=128,
                                                  durationTicks=256, offsetFrames=0, gain=1))
        self.assertTrue(any('overlaps' in problem for problem in measure.mechanical(song, meta['lanes'])))

    def test_pitch_changes_the_mechanical_source_duration_and_gap_direction(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'constant.wav'
            mixjam.write_wav(source, 44100, np.full((44100, 2), 0.25))
            for reverse in (False, True):
                for cents, duration, has_gap in ((-1200, 256, False), (1200, 128, True)):
                    with self.subTest(reverse=reverse, cents=cents):
                        song = mixjam.Song('pitched duration')
                        song.clip(1, source, 0, duration, reverse=reverse)
                        document = song.document()
                        document['lanes'][0]['automation'] = {'pitch': [{'tick': 0, 'value': cents}]}
                        problems = measure.mechanical(document, {})
                        gaps = [problem for problem in problems if 'silent gap' in problem]
                        self.assertEqual(bool(gaps), has_gap)
                        if has_gap:
                            self.assertIn('at its start' if reverse else 'at its end', gaps[0])
                            self.assertIn('0.21 bars', gaps[0])

    def test_pitch_fall_before_a_clip_changes_its_cut_status_and_fade_check(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'decaying.wav'
            data = np.full((44100, 2), 0.25)
            data[-4410:] = 0
            mixjam.write_wav(source, 44100, data)
            song = mixjam.Song('pitch fall')
            clip = song.clip(1, source, 128, 149)
            document = song.document()
            roles = {'1': {'name': 'Pad', 'role': 'pad'}}
            self.assertEqual(measure.mechanical(document, roles), [])
            # The fall crosses the placement's start. At the normal source end,
            # the slowed read is still inside the source, so the clip cuts it.
            document['lanes'][0]['automation'] = {'pitch': [
                {'tick': 64, 'value': 0}, {'tick': 256, 'value': -2400}]}
            problems = measure.mechanical(document, roles)
            self.assertEqual(len(problems), 1)
            self.assertIn('The clip is cut short.', problems[0])
            document['lanes'][0]['placements'][0]['fades'] = {'out': {'ticks': 8, 'curve': 'linear'}}
            self.assertEqual(measure.mechanical(document, roles), [])

    def test_reverse_pitch_uses_the_last_output_window_for_the_end_level(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'quiet-start.wav'
            data = np.full((44100, 2), 0.25)
            data[:300] = 0
            mixjam.write_wav(source, 44100, data)
            song = mixjam.Song('reverse pitch')
            song.clip(1, source, 0, 64, reverse=True)
            document = song.document()
            roles = {'1': {'name': 'Pad', 'role': 'pad'}}
            self.assertTrue(any('without a fade out' in p for p in measure.mechanical(document, roles)))
            # At 1/16 speed the last 50 ms read only the quiet first 138 frames.
            document['lanes'][0]['automation'] = {'pitch': [{'tick': 0, 'value': -4800}]}
            self.assertEqual(measure.mechanical(document, roles), [])

    def test_forward_pitch_checks_the_output_window_at_source_exhaustion(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'quiet-tail.wav'
            roles = {'1': {'name': 'Pad', 'role': 'pad'}}
            for cents, quiet_frames, duration, loud_end in ((-4800, 300, 2048, False),
                                                            (2400, 3000, 48, True)):
                with self.subTest(cents=cents):
                    data = np.full((44100, 2), 0.25)
                    data[-quiet_frames:] = 0
                    mixjam.write_wav(source, 44100, data)
                    song = mixjam.Song('forward pitch end', bpm=120)
                    song.clip(1, source, 0, duration)
                    document = song.document()
                    document['lanes'][0]['automation'] = {'pitch': [{'tick': 0, 'value': cents}]}
                    problems = measure.mechanical(document, roles)
                    # At 1/16 speed the final 50 ms read only quiet frames.
                    # At 4x speed they include loud frames, even though the
                    # source ends before the placement's silent final 16 ticks.
                    endings = [p for p in problems if 'without a fade out' in p]
                    self.assertEqual(bool(endings), loud_end)

    def test_reversed_melodic_end_uses_the_trim_start_not_the_source_tail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            loud_start = root / 'loud-start.wav'
            quiet_start = root / 'quiet-start.wav'
            data = np.zeros((44100, 2))
            data[:22050] = 0.5
            mixjam.write_wav(loud_start, 44100, data)
            mixjam.write_wav(quiet_start, 44100, data[::-1])
            song = mixjam.Song('reverse endings', bpm=120)
            song.clip(1, loud_start, 0, 128, reverse=True)
            song.clip(2, quiet_start, 0, 128, reverse=True)
            roles = {'1': {'name': 'Loud start', 'role': 'pad'},
                     '2': {'name': 'Quiet start', 'role': 'pad'}}
            problems = measure.mechanical(song.document(), roles)
            self.assertTrue(any('track 1' in problem and 'without a fade out' in problem for problem in problems))
            self.assertFalse(any('track 2' in problem and 'without a fade out' in problem for problem in problems))
            # Trimming into silence moves the reversed clip's ending there.
            song.lanes[0]['placements'][0]['offsetFrames'] = 30000
            problems = measure.mechanical(song.document(), roles)
            self.assertFalse(any('track 1' in problem and 'without a fade out' in problem for problem in problems))
            self.assertTrue(any('track 1' in problem and 'start' in problem and 'silent' in problem
                                for problem in problems))

    def test_midi_notes_on_separate_channels_keep_their_own_durations(self):
        # C4 on channel 0 at tick 0, channel 1 at tick 96; then each note off.
        events = bytes([0, 0x90, 60, 100, 96, 0x91, 60, 100,
                        96, 0x80, 60, 0, 96, 0x81, 60, 0, 0, 0xff, 0x2f, 0])
        data = b'MThd' + struct.pack('>IHHH', 6, 0, 1, 96)
        data += b'MTrk' + struct.pack('>I', len(events)) + events
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'channels.mid'
            path.write_bytes(data)
            notes, ppq = midinotes.notes(path)
        self.assertEqual(ppq, 96)
        self.assertEqual(notes, [(0, 192, 60), (96, 288, 60)])

    def test_tonal_indices_of_a_flat_spectrum(self):
        top_pres, pres_box = tonal_indices([0.0] * len(THIRDS))
        # 8-16 kHz has 4 bands, 1.6-5 kHz 6, 250-630 Hz 5.
        self.assertAlmostEqual(top_pres, 10 * math.log10(4 / 6))
        self.assertAlmostEqual(pres_box, 10 * math.log10(6 / 5))

    def test_flags_and_misses_follow_the_target_ranges(self):
        self.assertEqual(flag(-11.5, (-12.5, -11.0)), '-11.5')
        self.assertEqual(flag(-10.0, (-12.5, -11.0)), '-10.0 !')
        self.assertEqual(misses({'loud_rms': -10.0, 'corr': 0.9}, {'loud_rms': [-12.5, -11.0], 'corr': [0.85, 0.95],
                                                                    'crest': [8, 11]}), ['loud_rms'])

    def test_album_profile_keeps_existing_targets_and_derives_new_ones(self):
        keys = ['loud_rms', 'loudest', 'peak', 'crest', 'low', 'sub45', 'mid', 'hats', 'box', 'corr', 'spread',
                'top_pres', 'pres_box']
        results = {f'{n:02d} Track {n}': dict({k: float(n) for k in keys}, thirds_db=[float(n)] * len(THIRDS))
                   for n in range(1, 6)}
        new = album_profile(results, ['Track 5'])
        self.assertEqual(new['references']['album median']['loud_rms'], 3.0)
        self.assertEqual(new['references']['Track 5']['crest'], 5.0)
        self.assertEqual(new['targets']['corr'], [1.4, 4.6])
        kept = album_profile(results, [], dict(name='mine', targets={'corr': [0.8, 0.9]}, song_targets=['x']))
        self.assertEqual((kept['name'], kept['targets'], kept['song_targets']), ('mine', {'corr': [0.8, 0.9]}, ['x']))
        with self.assertRaises(SystemExit):
            album_profile(results, ['Missing'])

    def test_heard_share_counts_cells_within_six_db(self):
        part = np.array([[1.0, 1.0, 0.0], [1.0, 1.0, 1.0]])
        rest = np.array([[1.0, 10.0, 1.0], [0.1, 100.0, 1.0]])
        self.assertAlmostEqual(heard_share(part, rest), 3 / 5)
        self.assertEqual(heard_share(np.zeros((2, 3)), rest), 0.0)


class HarmonyTests(unittest.TestCase):
    def test_keys_and_scales(self):
        self.assertEqual(harmony.parse_key('A minor'), (9, 'minor'))
        self.assertEqual(harmony.parse_key('Am'), (9, 'minor'))
        self.assertEqual(harmony.parse_key('A natural minor'), (9, 'minor'))
        self.assertEqual(harmony.parse_key('Bb major'), (10, 'major'))
        self.assertEqual(harmony.parse_key('C'), (0, 'major'))
        self.assertEqual(harmony.scale('A minor'), {9, 11, 0, 2, 4, 5, 7})
        self.assertEqual(harmony.scale('A minor'), harmony.scale('C major'))
        with self.assertRaises(ValueError):
            harmony.parse_key('H minor')

    def test_chords(self):
        self.assertEqual(harmony.parse_chord('Am'), (9, {9, 0, 4}))
        self.assertEqual(harmony.parse_chord('E'), (4, {4, 8, 11}))
        self.assertEqual(harmony.parse_chord('G7')[1], {7, 11, 2, 5})
        self.assertEqual(harmony.best_triad({9: 2, 0: 1, 4: 1}), 'Am')

    def test_clash_cost_allows_chord_tones_and_flags_semitones_above_them(self):
        self.assertEqual(harmony.clash_cost({9: 1, 0: 1, 4: 1}, 'Am', 'A minor'), 0)
        # G# is outside A natural minor but a tone of E major.
        self.assertEqual(harmony.clash_cost({8: 1}, 'E', 'A minor'), 0)
        self.assertEqual(harmony.clash_cost({8: 1}, 'Am', 'A minor'), 1)
        # F over Am lies a semitone above the chord's E; C over G above its B.
        self.assertEqual(harmony.clash_cost({5: 1, 9: 3}, 'Am', 'A minor'), 0.25)
        self.assertEqual(harmony.clash_cost({0: 1, 7: 1}, 'G', 'A minor'), 0.5)

    def test_shift_moves_minor_files_to_the_key_and_major_files_to_its_relative_major(self):
        self.assertEqual(harmony.shift_to('Ab minor', 'A minor'), 1)
        self.assertEqual(shift_for('Lead Loop 4 150 Abm'), (1, 'Abm'))
        self.assertEqual(shift_for('Lead Loop 7 150 Ab'), (4, 'Ab'))
        self.assertEqual(shift_for('Chord Loop 2', ['Chord Loop 2 120 B']), (1, 'B'))
        self.assertEqual(shift_for('Lead Loop 4 Abm', to='E minor'), (-4, 'Abm'))
        self.assertEqual(shift_for('No key here'), (None, '?'))

    def test_fits_need_a_low_worst_bar_and_mean(self):
        bars = [{9: 4}, {5: 4}, {9: 4}, {7: 4}]
        self.assertEqual(fits(bars, {'Am F Am G': ['Am', 'F', 'Am', 'G']}, 'A minor'), [('Am F Am G', 1, 0.0)])
        # F held over the Am bar clashes for the whole bar.
        clashing = [{5: 4}, {5: 4}, {9: 4}, {7: 4}]
        self.assertEqual(fits(clashing, {'Am F Am G': ['Am', 'F', 'Am', 'G']}, 'A minor'), [])


class SampleIndexTests(unittest.TestCase):
    def test_index_refreshes_pack_metadata_and_subsecond_wav_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / 'index.tsv'
            sound = root / 'Deep 16.wav'
            pack = root / 'BPM.INF'

            def write_sound(rate):
                with wave.open(str(sound), 'wb') as output:
                    output.setnchannels(1)
                    output.setsampwidth(2)
                    output.setframerate(rate)
                    output.writeframes(bytes(44100 * 2))

            write_sound(44100)
            os.utime(sound, ns=(1700000000100000000, 1700000000100000000))
            pack.write_text('BPM=120\nScale=c\nSample6=Gis\nSample16=Dis\n')
            with mock.patch.object(samples, 'libraries', return_value=[root]), \
                    mock.patch.object(samples, 'index_path', return_value=target), \
                    mock.patch('sys.stdout', new_callable=io.StringIO), \
                    mock.patch('sys.stderr', new_callable=io.StringIO):
                samples.build(1)
                first = samples.load(target)[0]
                self.assertEqual(first['chord'], 'D#')
                with mock.patch.object(samples, 'wav_header', wraps=samples.wav_header) as probe:
                    samples.build(1)
                    probe.assert_not_called()
                pack.write_text('BPM=140\nScale=d\nSample16=Gis\n')
                samples.build(1)
                refreshed = samples.load(target)[0]
                self.assertEqual((refreshed['bpm'], refreshed['key'], refreshed['chord']), ('140.0', 'Dm', 'G#'))
                write_sound(22050)
                os.utime(sound, ns=(1700000000900000000, 1700000000900000000))
                samples.build(1)
                self.assertEqual(samples.load(target)[0]['seconds'], '2.0')

    def test_invalid_pack_tempos_do_not_abort_indexing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / 'tone.wav'
            with wave.open(str(path), 'wb') as output:
                output.setnchannels(1)
                output.setsampwidth(2)
                output.setframerate(44100)
                output.writeframes(bytes(88200))
            for value in ('nan', 'inf', '-120', '0', '301'):
                with self.subTest(value=value):
                    (root / 'BPM.INF').write_text(f'BPM={value}\n')
                    row = samples.describe(root, path.name, path.stat().st_size, 0, samples.Packs(root))
                    self.assertIsNone(row['bpm'])

    def test_names_give_tempo_key_and_root_note_in_each_common_style(self):
        cases = {
            'BASS001_140_A_SL1': (140.0, 'A', None),
            'Lead Loop 4 150 Abm': (150.0, 'G#m', None),
            'Calm - 137 BPM F# Maj Vocal': (137.0, 'F#', None),
            'Figure - Fmin - 135 BPM': (135.0, 'Fm', None),
            'warped_loop_90_Em': (90.0, 'Em', None),
            'Melody_Loop_10_124BPM_Amin_Lead': (124.0, 'Am', None),
            'Synth_Shot_45_G': (None, None, 'G'),
            'Synths Root C 011': (None, None, 'C'),
            'Drone Bass A 1': (None, None, None),
            'FM Bass': (None, None, None),
        }
        for name, expected in cases.items():
            self.assertEqual(samples.parse_name(name), expected, name)

    def test_typed_keys_and_chord_tables_use_one_spelling(self):
        self.assertEqual([samples.parse_key(k) for k in ('A min', 'f#m', 'Gb', 'Bbm', 'x')], ['Am', 'F#m', 'F#', 'A#m', None])
        self.assertEqual([samples.german_chord(c) for c in ('c', 'Dis', 'Gis', 'H', 'es')], ['Cm', 'D#', 'G#', 'B', 'D#m'])

    def test_header_gives_length_and_loop_tempo_and_the_pack_gives_chords(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Pack').mkdir()
            (root / 'Pack' / 'BPM.INF').write_text('[Tempo]\nBPM=140\n[HarmonyTable]\nScale=c\nSample1=c\nSample6=Gis\n')
            frames = 4 * 44100 * 240 // 140  # 4 bars at 140 BPM
            fmt = struct.pack('<HHIIHH', 1, 2, 44100, 44100 * 4, 4, 16)
            acid = struct.pack('<IHHfIHHf', 0, 0, 0x8000, 0.0, 16, 4, 4, 140.0)
            body = b'WAVE' + b'fmt ' + struct.pack('<I', 16) + fmt + b'acid' + struct.pack('<I', 24) + acid
            body += b'data' + struct.pack('<I', frames * 4) + bytes(frames * 4)
            path = root / 'Pack' / 'Deep 6.wav'
            path.write_bytes(b'RIFF' + struct.pack('<I', len(body)) + body)
            row = samples.describe(root, 'Pack/Deep 6.wav', path.stat().st_size, 0, samples.Packs(root))
        self.assertEqual((row['channels'], row['rate'], row['bpm'], row['bpm_from']), (2, 44100, 140.0, 'pack'))
        self.assertEqual((row['key'], row['chord'], row['beats'], row['bars']), ('Cm', 'G#', 16, 4.0))

    def test_whole_bars_needs_a_whole_number_of_beats(self):
        self.assertEqual(samples.whole_bars(240 / 140 * 2, 140), 2.0)
        self.assertIsNone(samples.whole_bars(1.3, 140))


if __name__ == '__main__':
    unittest.main()

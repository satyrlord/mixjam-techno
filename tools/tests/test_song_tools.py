"""Checks for the song tools in tools/song: settings, ticks, automation, harmony, and measures."""
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


class SongTests(unittest.TestCase):
    def test_explicit_pedal_power_overrides_the_named_pedal_default(self):
        for enabled in (False, True):
            board = mixjam.board(limiter={'on': enabled, 'gain': 3.0})
            self.assertIs(board['limiter']['on'], enabled)
            self.assertEqual(board['limiter']['gain'], 3.0)

    def test_eq_power_stays_boolean_in_the_song(self):
        for enabled in (False, True):
            song = mixjam.Song('test')
            song.eq(1, on=enabled, midDb=-3)
            self.assertIs(song.document()['lanes'][0]['eq']['on'], enabled)

    def test_ticks_count_bars_and_beats_from_one(self):
        self.assertEqual(mixjam.tick(1), 0)
        self.assertEqual(mixjam.tick(2), 256)
        self.assertEqual(mixjam.tick(1, 2), 64)
        self.assertEqual(mixjam.tick(3, 1.5), 544)
        self.assertEqual(mixjam.tick(1.5), 128)

    def test_ticks_follow_the_source_tempo(self):
        self.assertAlmostEqual(mixjam.ticks_of(44100 * 240 / 140, 44100, 140), 256)
        self.assertAlmostEqual(mixjam.ticks_of(44100 * 240 / 150, 44100, 150), 256)

    def test_loop_bars_accept_three_percent(self):
        self.assertEqual(mixjam.loop_bars(4.1), 4)
        self.assertIsNone(mixjam.loop_bars(4.2))
        self.assertEqual(mixjam.loop_bars(0.98), 1)

    def test_automation_merges_sections_and_refuses_a_second_point(self):
        song = mixjam.Song('test')
        song.automate(1, 'volume', [(9, -6), (17, 0)])
        song.automate(1, 'volume', [(1, 0), (5, -3)])
        self.assertEqual([p['tick'] for p in song.automation[0]['volume']], [0, 1024, 2048, 4096])
        with self.assertRaises(ValueError):
            song.automate(1, 'volume', [(5, 0)])
        song.automate(1, 'send2', [(1, 0.5)])
        self.assertEqual(song.automation[0]['sends'][1], [{'tick': 0, 'value': 0.5}])

    def test_hold_rests_at_neutral_until_the_first_point(self):
        held = mixjam.hold([{'tick': 1024, 'value': -6.0}, {'tick': 2048, 'value': 0.0}], 0.0, [], 'lane')
        self.assertEqual(held[:2], [{'tick': 0, 'value': 0.0}, {'tick': 1023, 'value': 0.0}])
        self.assertEqual(len(held), 4)

    def test_hold_refuses_a_lane_that_ends_off_neutral_over_a_later_clip(self):
        entries = [{'tick': 0, 'value': 0.0}, {'tick': 1024, 'value': -12.0}]
        self.assertEqual(len(mixjam.hold(entries, 0.0, [{'startTick': 512}], 'lane')), 2)
        with self.assertRaisesRegex(ValueError, 'bar 9'):
            mixjam.hold(entries, 0.0, [{'startTick': 2048}], 'lane')

    def test_fades_fit_inside_the_clip(self):
        placement = {'durationTicks': 100}
        mixjam.set_fade(placement, 'in', 80, 'linear')
        mixjam.set_fade(placement, 'out', 80, 'exponential')
        self.assertEqual(placement['fades']['in']['ticks'] + placement['fades']['out']['ticks'], 100)

    def test_board_switches_on_named_pedals_and_keeps_the_limiter_last(self):
        board = mixjam.board(trim=-3, imaging=dict(width=100), limiter=dict(gain=4.5))
        self.assertEqual(board['order'][-1], 'limiter')
        self.assertTrue(board['imaging']['on'])
        self.assertEqual(board['imaging']['monoBelow'], 120.0)
        self.assertFalse(board['tube']['on'])
        off = mixjam.board_off(board)
        self.assertEqual(off['trim'], 0.0)
        self.assertFalse(any(off[name]['on'] for name in off['order']))
        self.assertTrue(board['limiter']['on'])
        with self.assertRaises(ValueError):
            mixjam.board(reverb={})

    def test_bus_keeps_the_module_unchanged(self):
        song = mixjam.Song('test')
        module = mixjam.reverb('Dark Hall')
        song.bus(1, module)
        self.assertIn('_return', module)
        self.assertEqual(song.fx[0]['returnLevel'], 0.92)
        self.assertNotIn('_return', song.fx[0]['module'])


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
            def fake_render(source, output):
                Path(output).touch()
            with mock.patch.object(render, 'run', side_effect=fake_render), \
                    mock.patch.object(render, 'read_wav', return_value=(44100, np.full((8, 2), 0.01))):
                with self.assertRaisesRegex(SystemExit, 'gain'):
                    render.fit_gain(song, root, -0.5)
            self.assertEqual(song.read_text(encoding='utf-8'), original)

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

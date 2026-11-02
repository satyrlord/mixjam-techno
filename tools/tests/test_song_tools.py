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
import barnotes  # noqa: E402
import harmony  # noqa: E402
import mixjam  # noqa: E402
import master  # noqa: E402
import measure  # noqa: E402
import render  # noqa: E402
import midinotes  # noqa: E402
import samples  # noqa: E402
from audible import heard_share, lane_bars  # noqa: E402
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
    def test_song_destinations_reject_invalid_indices_before_changes(self):
        actions = (
            lambda song, track: song.lane(track, 'pad', 'pad'),
            lambda song, track: song.eq(track, midDb=-3),
            lambda song, track: song.clip(track, 'unread.wav', 0, 256),
            lambda song, track: song.loop(track, 'unread.wav', 1, 4),
            lambda song, track: song.phrase(track, 'unread.wav', 1),
            lambda song, track: song.automate(track, 'volume', [(1, 0)]),
        )
        for track in (0, -1, 65, True, 1.5, float('nan')):
            for action in actions:
                song = mixjam.Song('indices')
                before = json.dumps((song.lanes, song.automation, song.samples, song.names, song.roles))
                with self.subTest(track=track, action=actions.index(action)):
                    with mock.patch.object(song, 'sound', side_effect=AssertionError('Source was read')):
                        with self.assertRaises(ValueError):
                            action(song, track)
                    self.assertEqual(json.dumps((song.lanes, song.automation, song.samples,
                                                 song.names, song.roles)), before)
                    self.assertFalse(song.names)
                    self.assertFalse(song.roles)
        for index in (0, -1, 5, True, 1.5, float('nan')):
            song = mixjam.Song('bus indices')
            before = json.dumps(song.fx)
            with self.subTest(bus=index), self.assertRaises(ValueError):
                song.bus(index, mixjam.delay())
            self.assertEqual(json.dumps(song.fx), before)

    def test_rejected_automation_batch_preserves_previous_points(self):
        for lane in ('volume', 'send2'):
            song = mixjam.Song('automation')
            song.automate(1, lane, [(1, 0), (9, 0)])
            before = json.dumps(song.automation)
            with self.subTest(lane=lane), self.assertRaises(ValueError):
                song.automate(1, lane, [(5, 0.5), (9, 0)])
            self.assertEqual(json.dumps(song.automation), before)
        for lane in ('send0', 'send5', 'send-1', 'gain', 'balance', ''):
            song = mixjam.Song('automation destination')
            before = json.dumps(song.automation)
            with self.subTest(lane=lane), self.assertRaises(ValueError):
                song.automate(1, lane, [(1, 0)])
            self.assertEqual(json.dumps(song.automation), before)

    def test_loop_rejects_nonadvancing_tiles_before_placement(self):
        sound = mock.Mock(loop_bars=1, bars=1.0, ticks=256)
        for first, last, size in ((1, 4, -1), (1, 4, 0), (1, 4, float('nan')),
                                  (1, 4, float('inf')), (1, 4, 1 / 1024),
                                  (5, 4, 1), (0, 4, 1)):
            song = mixjam.Song('loop')
            with self.subTest(first=first, last=last, size=size):
                with mock.patch.object(song, 'sound', return_value=sound), \
                        mock.patch.object(song, 'clip', side_effect=AssertionError('Clip was placed')):
                    with self.assertRaises(ValueError):
                        song.loop(1, 'loop.wav', first, last, size=size)

    def test_automatic_end_fade_hears_opposite_channel_polarities(self):
        rate = 12000
        data = np.full((rate, 2), 0.25)
        data[-rate // 10:, 1] *= -1
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'stereo.wav'
            mixjam.write_wav(path, rate, data)
            song = mixjam.Song('end fade')
            song.lane(1, 'pad', 'pad')
            self.assertTrue(song.sound(path).ends_loud)
            clip = song.phrase(1, path, 1)
            self.assertEqual(clip['fades']['out']['ticks'], 1)

    def test_ducker_rejects_invalid_settings_without_changing_the_lane(self):
        song = mixjam.Song('ducker')
        for track, key, settings in ((0, 2, {}), (65, 2, {}), (1, 0, {}), (1, 65, {}),
                                     (1, 1.5, {}), (1, 1, {}), (1, 2, {'depth': 25}),
                                     (1, 2, {'attack': float('nan')}), (1, 2, {'release': 9})):
            before = json.dumps(song.document()['lanes'])
            with self.subTest(track=track, key=key, settings=settings):
                with self.assertRaises(ValueError):
                    song.ducker(track, key, **settings)
                self.assertEqual(json.dumps(song.document()['lanes']), before)

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

    def test_sounds_play_in_the_octave_nearest_the_song_tempo(self):
        for source, song, expected in ((90, 180, 180), (90, 160, 180), (90, 120, 90), (80, 160, 160),
                                       (45, 180, 180), (180, 120, 90), (180, 130, 180), (140, 120, 140)):
            with self.subTest(source=source, song=song):
                self.assertEqual(mixjam.playback_bpm(source, song), expected)
        rate = 44100
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'half-time.wav'
            mixjam.write_wav(path, rate, np.zeros((rate * 240 // 90, 2)))  # one bar at 90 BPM
            song = mixjam.Song('half-time', bpm=180)
            song.tempo(path, 90)
            self.assertAlmostEqual(song.sound(path).bars, 2.0)
            self.assertEqual([p['durationTicks'] for p in song.loop(1, path, 1, 4)], [512, 512])

    def test_ticks_count_bars_and_beats_from_one(self):
        self.assertEqual(mixjam.tick(1), 0)
        self.assertEqual(mixjam.tick(2), 256)
        self.assertEqual(mixjam.tick(1, 2), 64)
        self.assertEqual(mixjam.tick(3, 1.5), 544)
        self.assertEqual(mixjam.tick(1.5), 128)

    def test_bar_notes_follow_the_bars_played_at_the_song_tempo(self):
        rate = 12000
        frames = rate * 240 // 90
        first = np.sin(2 * np.pi * 220 * np.arange(frames // 2) / rate)
        second = np.sin(2 * np.pi * 261.625565 * np.arange(frames // 2) / rate)
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'half-time.wav'
            mixjam.write_wav(path, rate, np.r_[first, second][:, None])
            for source_bpm, song_bpm, count in ((90, 180, 2), (180, 120, 1)):
                with self.subTest(source=source_bpm, song=song_bpm):
                    output = io.StringIO()
                    args = ['barnotes.py', '--song-bpm', str(song_bpm), str(path), str(source_bpm), '0']
                    with mock.patch.object(sys, 'argv', args), mock.patch('sys.stdout', output):
                        barnotes.main()
                    bars = [line for line in output.getvalue().splitlines() if line.startswith('  bar ')]
                    self.assertEqual(len(bars), count)
                    if source_bpm == 90:
                        self.assertIn('root A ', bars[0])
                        self.assertIn('notes A', bars[0])
                        self.assertIn('root C ', bars[1])
                        self.assertIn('notes C', bars[1])

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

    def test_pan_automation_sweeps_from_the_knob_and_checks_its_range(self):
        song = mixjam.Song('pan')
        song.lane(1, 'ratchet', 'perc', pan=0.25)
        # Sixteenths from bar 2: left, right, and back to the knob.
        song.automate(1, 'pan', [(2, -1), (2.0625, 1), (2.125, 0.25)])
        document = song.document()
        self.assertEqual(document['formatVersion'], 24)
        self.assertEqual(document['lanes'][0]['pan'], 0.25)
        # The lane rests at the knob until its first point.
        self.assertEqual(document['lanes'][0]['automation']['pan'],
                         [dict(tick=0, value=0.25), dict(tick=255, value=0.25),
                          dict(tick=256, value=-1.0), dict(tick=272, value=1.0),
                          dict(tick=288, value=0.25)])
        before = json.dumps(song.automation)
        for value in (-1.01, 1.5, float('nan')):
            with self.subTest(value=value), self.assertRaises(ValueError):
                song.automate(1, 'pan', [(3, 0), (4, value)])
            self.assertEqual(json.dumps(song.automation), before)
        # A lane that ends away from the knob while a later clip plays is refused.
        song.automate(1, 'pan', [(4, 1)])
        song.lanes[0]['placements'].append(dict(startTick=5 * 256))
        with self.assertRaises(ValueError):
            song.document()

    def test_repeat_automation_steps_from_off_and_checks_its_values(self):
        song = mixjam.Song('repeat')
        song.lane(1, 'cello', 'lead')
        # A roll from bar 2: 1/8, 1/16, 1/32, then off.
        song.automate(1, 'repeat', [(2, 8), (2.25, 16), (2.375, 32), (2.5, 0)])
        song.automate(1, 'repeat', [(4, 4), (4.5, 64), (4.75, 0)])
        document = song.document()
        self.assertEqual(document['formatVersion'], 24)
        # The lane is off before its first point, so no resting points are added.
        self.assertEqual(document['lanes'][0]['automation']['repeat'],
                         [dict(tick=256, value=8.0), dict(tick=320, value=16.0),
                          dict(tick=352, value=32.0), dict(tick=384, value=0.0),
                          dict(tick=768, value=4.0), dict(tick=896, value=64.0),
                          dict(tick=960, value=0.0)])
        before = json.dumps(song.automation)
        for value in (12, 3, 128, -8, 16.5, 2, float('nan'), True):
            with self.subTest(value=value), self.assertRaises(ValueError):
                song.automate(1, 'repeat', [(6, 16), (7, value)])
            self.assertEqual(json.dumps(song.automation), before)
        # A lane that ends on a division while a later clip plays is refused.
        song.automate(1, 'repeat', [(8, 16)])
        song.lanes[0]['placements'].append(dict(startTick=9 * 256))
        with self.assertRaises(ValueError):
            song.document()

    def test_lane_cache_accepts_format_21_and_keys_the_repeat_lane(self):
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            root = Path(directory)
            source = root / 'source.wav'
            mixjam.write_wav(source, 44100, np.ones((8, 2)))
            song = mixjam.Song('repeat cache')
            song.clip(1, source, 0, 96)
            document = song.document()
            self.assertEqual(document['formatVersion'], 24)
            self.assertIsNotNone(render.source_fingerprints(document))
            key = render.lane_key(document, [1], False)
            document['lanes'][0]['automation'] = {'repeat': [dict(tick=0, value=16.0), dict(tick=64, value=0.0)]}
            self.assertNotEqual(render.lane_key(document, [1], False), key)
            document['formatVersion'] = 25
            self.assertIsNone(render.source_fingerprints(document))

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

    def test_sections_are_saved_in_the_song_in_time_order(self):
        song = mixjam.Song('sections')
        self.assertNotIn('sections', song.document()['song'])
        song.section('Drift', 9, 16)
        song.section('Intro', 1, 8)
        self.assertEqual(song.document()['song']['sections'],
                         [dict(startTick=0, name='Intro'), dict(startTick=8 * 256, name='Drift')])
        for name in ('', ' Intro', 'a\nb', '\ud800', '\udfff', 'x' * 49):
            with self.assertRaises(ValueError):
                song.section(name, 17, 8)
        song.section('Again', 9, 4)
        with self.assertRaises(ValueError):
            song.document()

    def test_second_peak_keys_are_written_only_when_they_differ(self):
        song = mixjam.Song('peak 2')
        song.eq(1, midDb=-3)
        self.assertFalse({'peak2Hz', 'peak2Db', 'peak2Q'} & set(song.lanes[0]['eq']))
        song.eq(2, peak2Hz=191, peak2Db=-9, peak2Q=8)
        eq = song.lanes[1]['eq']
        self.assertEqual((eq['peak2Hz'], eq['peak2Db'], eq['peak2Q']), (191.0, -9.0, 8.0))
        song.eq(3, peak2Db=-6)
        self.assertNotIn('peak2Hz', song.lanes[2]['eq'])
        for settings in ({'peak2Hz': 39}, {'peak2Hz': 16001}, {'peak2Q': 0.2}, {'peak2Db': 19}):
            with self.subTest(settings=settings), self.assertRaises(ValueError):
                song.eq(4, **settings)
        self.assertNotIn('eq', song.lanes[3])
        self.assertEqual(song.document()['formatVersion'], 24)

    def test_drive_checks_its_ranges_and_saves_as_the_app_does(self):
        song = mixjam.Song('drive')
        song.drive(1, amount=18, mix=40, focus=98)
        self.assertEqual(song.lanes[0]['drive'], dict(on=True, amount=18.0, character=25.0, mix=40.0,
                                                      focus=98.0, output=0.0))
        document = song.document()
        self.assertEqual(document['formatVersion'], 24)
        self.assertEqual(document['lanes'][0]['drive']['focus'], 98.0)
        for settings in ({'amount': -1}, {'amount': 25}, {'character': 101}, {'mix': -0.5}, {'mix': 101},
                         {'focus': 19}, {'focus': 2001}, {'output': 12.5}, {'output': float('nan')}):
            with self.subTest(settings=settings), self.assertRaises(ValueError):
                song.drive(2, **settings)
        self.assertNotIn('drive', song.lanes[1])
        # A drive that is off keeps its settings; off at the defaults it is gone.
        song.drive(1, amount=6, on=False)
        self.assertEqual(song.lanes[0]['drive']['amount'], 6.0)
        song.drive(1, on=False)
        self.assertNotIn('drive', song.lanes[0])
        for track in (0, 65, 1.5):
            with self.subTest(track=track), self.assertRaises(ValueError):
                song.drive(track)

    def test_lane_cache_keys_the_drive(self):
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            root = Path(directory)
            source = root / 'source.wav'
            mixjam.write_wav(source, 44100, np.ones((8, 2)))
            song = mixjam.Song('drive cache')
            song.clip(1, source, 0, 96)
            document = song.document()
            self.assertIsNotNone(render.source_fingerprints(document))
            key = render.lane_key(document, [1], False)
            song.drive(1, amount=18)
            driven = song.document()
            self.assertEqual(driven['formatVersion'], 24)
            self.assertNotEqual(render.lane_key(driven, [1], False), key)

    def test_repeat_gate_steps_without_resting_points_and_checks_its_range(self):
        song = mixjam.Song('gate')
        song.automate(1, 'repeat', [(2, 16), (3, 0)])
        song.automate(1, 'repeatgate', [(2, 25), (2.5, 60)])
        song.automate(1, 'repeatgate', [(4, 5)])
        document = song.document()
        self.assertEqual(document['formatVersion'], 24)
        # The gate is full before its first point and acts only while repeat is
        # on, so it needs no resting points and may end anywhere.
        self.assertEqual(document['lanes'][0]['automation']['repeatGate'],
                         [dict(tick=256, value=25.0), dict(tick=384, value=60.0), dict(tick=768, value=5.0)])
        for value in (4.9, 100.5, -1):
            with self.subTest(value=value), self.assertRaises(ValueError):
                song.automate(2, 'repeatgate', [(1, value)])
        with self.assertRaises(ValueError):
            song.automate(1, 'repeatgate', [(2, 50)])
        with self.assertRaises(ValueError):
            song.automate(1, 'repeatGate', [(5, 50)])

    def test_pitch_rests_at_zero_without_resting_points_and_checks_its_range(self):
        song = mixjam.Song('tape-stop')
        song.automate(1, 'pitch', [(1.5, 0), (2, -4800), (2.0625, 0)])
        document = song.document()
        self.assertEqual(document['formatVersion'], 24)
        # The lane is 0 before its first point, so no resting points are added.
        self.assertEqual(document['lanes'][0]['automation']['pitch'],
                         [dict(tick=128, value=0.0), dict(tick=256, value=-4800.0),
                          dict(tick=272, value=0.0)])
        for value in (-4801, 2400.5, True, float('nan')):
            with self.subTest(value=value), self.assertRaises(ValueError):
                song.automate(2, 'pitch', [(1, value)])
        with self.assertRaises(ValueError):
            song.automate(1, 'pitch', [(2, -1200)])
        # A lane held away from 0 over a later clip of its track is an error.
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'tape.wav'
            mixjam.write_wav(source, 44100, np.ones((8, 2)) * 0.25)
            held = mixjam.Song('held')
            held.clip(1, source, 0, 256)
            held.clip(1, source, 512, 256)
            held.automate(1, 'pitch', [(1.5, 0), (2, -1200)])
            with self.assertRaisesRegex(ValueError, 'pitch'):
                held.document()

    def test_lane_cache_keys_the_pitch_lane(self):
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            root = Path(directory)
            source = root / 'source.wav'
            mixjam.write_wav(source, 44100, np.ones((8, 2)))
            song = mixjam.Song('pitch cache')
            song.clip(1, source, 0, 96)
            document = song.document()
            self.assertEqual(document['formatVersion'], 24)
            self.assertIsNotNone(render.source_fingerprints(document))
            key = render.lane_key(document, [1], False)
            document['lanes'][0]['automation'] = {'pitch': [dict(tick=0, value=0.0), dict(tick=64, value=-4800.0)]}
            self.assertNotEqual(render.lane_key(document, [1], False), key)

    def test_return_lanes_save_on_their_bus_and_check_their_range(self):
        song = mixjam.Song('throw')
        song.bus(3, mixjam.delay(), return_level=0.3)
        song.automate_return(3, [(16.75, 0.3), (17, 1.8)])
        song.automate_return(3, [(18.5, 0.3)])
        document = song.document()
        self.assertEqual(document['formatVersion'], 24)
        self.assertEqual(document['fxBuses'][2]['returnAutomation'],
                         [dict(tick=4032, value=0.3), dict(tick=4096, value=1.8), dict(tick=4480, value=0.3)])
        self.assertEqual(document['fxBuses'][2]['returnLevel'], 0.3)
        self.assertNotIn('returnAutomation', document['fxBuses'][0])
        # A later bus() keeps the lane: it belongs to the bus slot, not its effect.
        song.bus(3, {'type': 'empty'})
        self.assertEqual(len(song.document()['fxBuses'][2]['returnAutomation']), 3)
        for index, value in ((3, 2.01), (3, -0.1), (3, True), (5, 1.0), (0, 1.0)):
            with self.subTest(index=index, value=value), self.assertRaises(ValueError):
                song.automate_return(index, [(30, value)])
        with self.assertRaises(ValueError):
            song.automate_return(3, [(17, 1.0)])

    def test_reverse_clips_save_only_when_reversed(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'swell.wav'
            mixjam.write_wav(source, 44100, np.ones((44100, 2)) * 0.25)
            song = mixjam.Song('swell')
            forward = song.clip(1, source, 0, 128)
            backward = song.clip(1, source, 128, 128, offset_frames=100, fade_in=16, reverse=True)
            self.assertNotIn('reverse', forward)
            self.assertIs(backward['reverse'], True)
            self.assertEqual(backward['fades']['in']['ticks'], 16)
            document = song.document()
            self.assertEqual([p.get('reverse') for p in document['lanes'][0]['placements']], [None, True])

    def test_lane_cache_keys_the_gate_the_return_lane_and_reverse(self):
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(render, 'exe_stamp', return_value='renderer-v1'):
            root = Path(directory)
            source = root / 'source.wav'
            mixjam.write_wav(source, 44100, np.ones((8, 2)))
            song = mixjam.Song('tools cache')
            song.clip(1, source, 0, 96)
            document = song.document()
            self.assertIsNotNone(render.source_fingerprints(document))
            key = render.lane_key(document, [1], False)
            keys = {key}
            changed = json.loads(json.dumps(document))
            changed['lanes'][0]['automation'] = {'repeatGate': [dict(tick=0, value=50.0)]}
            keys.add(render.lane_key(changed, [1], False))
            changed = json.loads(json.dumps(document))
            changed['fxBuses'][0]['returnAutomation'] = [dict(tick=0, value=1.5)]
            keys.add(render.lane_key(changed, [1], False))
            changed = json.loads(json.dumps(document))
            changed['lanes'][0]['placements'][0]['reverse'] = True
            keys.add(render.lane_key(changed, [1], False))
            self.assertEqual(len(keys), 4)

    def test_compressor_and_width_check_their_ranges(self):
        song = mixjam.Song('dynamics')
        song.compressor(1, threshold=-24, ratio=6, attack=0.5, makeup=3)
        self.assertEqual(song.lanes[0]['compressor'], dict(on=True, threshold=-24.0, ratio=6.0, attack=0.5,
                                                           release=100.0, makeup=3.0))
        for settings in ({'threshold': -61}, {'ratio': 0.9}, {'ratio': 21}, {'attack': 0.05},
                         {'release': 1001}, {'makeup': 25}, {'makeup': float('nan')}):
            with self.subTest(settings=settings), self.assertRaises(ValueError):
                song.compressor(2, **settings)
        self.assertNotIn('compressor', song.lanes[1])
        song.width(3, 40)
        self.assertEqual(song.lanes[2]['width'], 40.0)
        song.width(3, 100)
        self.assertNotIn('width', song.lanes[2])
        for track, percent in ((3, -1), (3, 201), (0, 50), (65, 50)):
            with self.subTest(track=track, percent=percent), self.assertRaises(ValueError):
                song.width(track, percent)

    def test_pedal_and_fx_keys_name_a_track_or_their_own_input(self):
        board = mixjam.board(limiter=dict(gain=4.0, key=3), busCompressor=dict(key=0))
        self.assertEqual(board['limiter']['key'], 3)
        self.assertNotIn('key', board['busCompressor'])
        mixjam.pedal_key(board, 'limiter', 0)
        self.assertNotIn('key', board['limiter'])
        for pedal, key in (('tube', 2), ('limiter', 65), ('limiter', -1), ('limiter', 1.5), ('limiter', True)):
            with self.subTest(pedal=pedal, key=key), self.assertRaises(ValueError):
                mixjam.pedal_key(board, pedal, key)
        song = mixjam.Song('keys')
        song.bus(1, mixjam.delay(duckKey=2))
        self.assertEqual(song.fx[0]['module']['duckKey'], 2)
        song.bus(2, mixjam.reverb(duckKey=0))
        self.assertNotIn('duckKey', song.fx[1]['module'])
        with self.assertRaises(ValueError):
            song.bus(3, mixjam.reverb(duckKey=65))
        song.master = mixjam.board(limiter=dict(key=4))
        song.ducker(5, 6)
        self.assertEqual(song.key_tracks(), {2, 4, 6})
        for track in range(7, 13):
            song.ducker(track, track + 20)
        with self.assertRaisesRegex(ValueError, 'key tracks'):
            song.document()

    def test_key_limit_uses_the_board_that_will_be_written(self):
        song = mixjam.Song('keys with bypassed board')
        for track in range(1, 9):
            song.ducker(track, track + 20)
        song.master = mixjam.board(limiter=dict(key=40))
        with self.assertRaisesRegex(ValueError, 'key tracks'):
            song.document()
        premaster = song.document(master=False)
        self.assertFalse(premaster['master']['limiter']['on'])
        self.assertTrue(song.master['limiter']['on'])
        song.master['bypass'] = True
        self.assertEqual(song.key_tracks(), set(range(21, 29)))
        self.assertTrue(song.document()['master']['bypass'])
        # Bypassing the board does not exempt active lane keys from the limit.
        song.ducker(9, 29)
        with self.assertRaisesRegex(ValueError, 'key tracks'):
            song.document()
        with self.assertRaisesRegex(ValueError, 'key tracks'):
            song.document(master=False)

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
            document['lanes'][index]['placements'] = [dict(id=index + 1)]
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

    def test_cache_files_are_independent_and_damaged_entries_are_misses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cache, source, output = root / 'cache', root / 'source.wav', root / 'output.wav'
            key = 'a' * 32
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.25))
            render.to_cache(cache, key, source)
            # Editing a delivered lane must not edit the cache, or vice versa.
            mixjam.write_wav(source, 44100, np.full((8, 2), 0.5))
            self.assertTrue(render.from_cache(cache, key, output))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.25)
            mixjam.write_wav(output, 44100, np.zeros((8, 2)))
            self.assertTrue(render.from_cache(cache, key, output))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.25)
            entry = cache / render.CACHE_VERSION / f'{key}.wav'
            entry.write_bytes(entry.read_bytes()[:-4])
            self.assertFalse(render.from_cache(cache, key, output))
            np.testing.assert_array_equal(mixjam.read_wav(output)[1], 0.25)
            entry.write_bytes(b'bad WAV')
            self.assertFalse(render.from_cache(cache, key, output))
            entry.unlink()
            self.assertFalse(render.from_cache(cache, key, output))

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

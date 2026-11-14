"""Checks for the song tools' song model: ticks, lanes, automation, and edits."""
import json
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import barnotes  # noqa: E402
import mixjam  # noqa: E402
import master  # noqa: E402
import render  # noqa: E402


class SongTests(unittest.TestCase):
    def test_format_version_matches_the_application(self):
        source = (Path(__file__).resolve().parents[2] / 'src' / 'mixjam' / 'song.rs').read_text(encoding='utf-8')
        line = next(line for line in source.splitlines() if 'const FORMAT_VERSION' in line)
        self.assertEqual(int(line.split('=')[1].strip(' ;')), mixjam.FORMAT_VERSION)

    def test_render_arguments_keep_flags_out_of_positionals(self):
        arguments = render.parse_arguments(['song.mixjam', 'out', '--jobs', '2', '--no-cache'])
        self.assertEqual((arguments.song, arguments.out, arguments.jobs), (Path('song.mixjam'), Path('out'), 2))
        self.assertTrue(arguments.no_cache)
        with mock.patch.object(sys, 'stderr', io.StringIO()), self.assertRaises(SystemExit):
            render.parse_arguments(['song.mixjam', 'out', '--cache', 'lanes', '--no-cache'])

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


if __name__ == '__main__':
    unittest.main()

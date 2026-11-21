"""Checks for the small shared helpers of the song tools: cli, wavio, lanes, midinotes, harmony, and mixjam."""
import copy
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
import wave
from unittest import mock

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import cli  # noqa: E402
import fit  # noqa: E402
import harmony  # noqa: E402
import lanes  # noqa: E402
import midichords  # noqa: E402
import midinotes  # noqa: E402
import mixjam  # noqa: E402
import wavio  # noqa: E402


def midi_file(path, events, ppq=96, division=None):
    """Write a one-track Standard MIDI file whose track holds the raw `events` bytes."""
    track = events + b'\x00\xff\x2f\x00'
    header = struct.pack('>IHHH', 6, 0, 1, ppq if division is None else division)
    path.write_bytes(b'MThd' + header + b'MTrk' + struct.pack('>I', len(track)) + track)
    return path


class CommandLineTests(unittest.TestCase):
    def test_option_converts_the_value_and_falls_back_to_the_default(self):
        args = ['--bpm', '140', 'a.wav']
        self.assertEqual(cli.option(args, '--bpm', float), 140.0)
        self.assertEqual(cli.option(args, '--to', default='A minor'), 'A minor')
        self.assertEqual(cli.options(['--x', '1', 'f', '--x', '2'], '--x', int), [1, 2])

    def test_pop_option_removes_the_flag_and_its_value(self):
        args = ['--to', 'E minor', 'folder']
        self.assertEqual(cli.pop_option(args, '--to'), 'E minor')
        self.assertEqual(args, ['folder'])
        self.assertEqual(cli.pop_option(args, '--to', default='x'), 'x')

    def test_a_flag_without_a_value_stops_with_a_message(self):
        for call in (lambda: cli.option(['--bpm'], '--bpm', float),
                     lambda: cli.options(['a', '--x'], '--x'),
                     lambda: cli.pop_option(['--to'], '--to'),
                     lambda: cli.option(['--out', '--bpm', '140'], '--out', Path),
                     lambda: cli.options(['--track', '--album'], '--track'),
                     lambda: cli.pop_option(['--to', '--quiet'], '--to')):
            with self.assertRaises(SystemExit) as caught:
                call()
            self.assertIn('needs a value', str(caught.exception))

        self.assertEqual(cli.option(['--fit', '-6'], '--fit', float), -6.0)

    def test_a_value_that_cannot_convert_names_the_flag(self):
        with self.assertRaises(SystemExit) as caught:
            cli.option(['--bpm', 'fast'], '--bpm', float)
        self.assertIn('--bpm', str(caught.exception))


class Pcm24Tests(unittest.TestCase):
    def test_unpack_reads_signed_little_endian_samples(self):
        raw = bytes.fromhex('000000' '010000' 'ffffff' 'ffff7f' '000080')
        self.assertEqual(wavio.unpack_pcm24(raw).tolist(), [0, 1, -1, 8388607, -8388608])

    def test_a_24_bit_wav_reads_to_floats(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'deep.wav'
            with wave.open(str(path), 'wb') as output:
                output.setnchannels(1)
                output.setsampwidth(3)
                output.setframerate(44100)
                output.writeframes(bytes.fromhex('000040' 'ffffbf'))
            rate, data = mixjam.read_wav(path)
        self.assertEqual(rate, 44100)
        np.testing.assert_allclose(data[:, 0], [0.5, -0.5], atol=1e-6)

    def test_a_wav_with_no_frames_is_not_a_sound(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'empty.wav'
            with wave.open(str(path), 'wb') as output:
                output.setnchannels(1)
                output.setsampwidth(2)
                output.setframerate(44100)
            with self.assertRaises(ValueError) as caught:
                mixjam.Sound(path, 140)
        self.assertIn('no audio', str(caught.exception))


class LaneTests(unittest.TestCase):
    def test_mix_down_sums_lanes_pads_the_shorter_and_reports_peaks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mixjam.write_wav(root / 'a.wav', 44100, np.full((8, 2), 0.5))
            mixjam.write_wav(root / 'b.wav', 44100, np.full((4, 2), 0.25))
            mixjam.write_wav(root / 'c.wav', 44100, np.zeros((2, 2)))
            peaks = {}
            rate, premaster, stems = lanes.mix_down(
                {'a': root / 'a.wav', 'b': root / 'b.wav', 'c': root / 'c.wav'},
                {'a': 'bass', 'b': 'bass'}, peaks)
        self.assertEqual(rate, 44100)
        self.assertEqual(premaster.shape, (8, 2))
        np.testing.assert_allclose(premaster[:, 0], [0.75] * 4 + [0.5] * 4, atol=1e-3)
        self.assertEqual(list(stems), ['bass'])
        self.assertAlmostEqual(peaks['a'], 0.5, places=3)
        self.assertAlmostEqual(peaks['b'], 0.25, places=3)
        self.assertEqual(peaks['c'], 0.0)

    def test_a_lane_and_its_twin_count_as_one_lane(self):
        document = {'lanes': [{'placements': [1]}, {'placements': [1]}, {'placements': []}]}
        meta = {'lanes': {'1': {'name': 'Bass', 'role': 'bass'}, '2': {'name': 'Bass (twin)', 'role': 'bass'}}}
        groups = lanes.lane_groups(document, meta)
        self.assertEqual(groups, {'Bass': [1, 2]})
        self.assertEqual(lanes.lane_roles(groups, meta), {'Bass': 'bass'})
        self.assertEqual(lanes.lane_groups(document, None), {'track 1': [1], 'track 2': [2]})


class MixjamHelperTests(unittest.TestCase):
    def test_twin_base_and_section_names(self):
        self.assertEqual(mixjam.twin_base('Bass (twin)'), 'Bass')
        self.assertEqual(mixjam.twin_base('Bass'), 'Bass')
        for name in ('Drop', 'Intro 2', 'Break - a'):
            self.assertTrue(mixjam.valid_section_name(name), name)
        for name in ('', ' Drop', 'Drop ', 'one\ntwo', 'x' * (mixjam.MAX_SECTION_NAME + 1), 7):
            self.assertFalse(mixjam.valid_section_name(name), repr(name))

    def test_automation_points_need_a_finite_bar_from_one_and_a_finite_value(self):
        song = mixjam.Song('points')
        for bar, value in ((0.5, 0), (0, 0), (-1, 0), (float('nan'), 0), (float('inf'), 0), (2, float('nan'))):
            with self.subTest(bar=bar, value=value), self.assertRaises(ValueError):
                song.automate(1, 'volume', [(bar, value)])
        song.automate(1, 'volume', [(1, 0), (2.5, -3)])
        self.assertEqual(mixjam.point_tick(1, 0, 'x'), 0)

    def test_automation_matches_the_native_timeline_and_control_ranges(self):
        song = mixjam.Song('points')
        end = mixjam.MAX_BARS + 1
        song.automate(1, 'volume', [(1, -60), (end, 6)])
        song.automate(1, 'filter', [(1, -1), (end, 1)])
        song.automate(1, 'send1', [(1, 0), (end, 1)])
        song.automate_return(1, [(1, 0), (end, 2)])
        before = copy.deepcopy((song.automation, song.returns))
        for lane, value in (('volume', -61), ('volume', 7), ('filter', -2), ('filter', 2),
                            ('send1', -0.1), ('send4', 1.1)):
            with self.subTest(lane=lane, value=value), self.assertRaises(ValueError):
                song.automate(1, lane, [(2, 0), (3, value)])
            self.assertEqual((song.automation, song.returns), before)
        with self.assertRaises(ValueError):
            song.automate(1, 'volume', [(2, 0), (end + 1, 0)])
        with self.assertRaises(ValueError):
            song.automate_return(1, [(2, 1), (end + 1, 1)])
        self.assertEqual((song.automation, song.returns), before)

    def test_atomic_write_replaces_the_file_and_keeps_it_whole_on_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / 'song.mixjam'
            mixjam.atomic_write_text(target, 'first')
            mixjam.atomic_write_text(target, 'second')
            self.assertEqual(target.read_text(encoding='utf-8'), 'second')
            with mock.patch.object(mixjam.os, 'replace', side_effect=OSError('interrupted')):
                with self.assertRaises(OSError):
                    mixjam.atomic_write_text(target, 'third')
            self.assertEqual(target.read_text(encoding='utf-8'), 'second')
            self.assertEqual(list(root.glob('*.tmp')), [])

    def test_atomic_write_removes_its_temporary_file_when_close_fails(self):
        real_temporary_file = mixjam.NamedTemporaryFile

        def fail_close(*args, **kwargs):
            file = real_temporary_file(*args, **kwargs)
            wrapper = mock.MagicMock(wraps=file)
            wrapper.name = file.name
            wrapper.__enter__.return_value = file

            def close(*_):
                file.close()
                raise OSError('flush failed')

            wrapper.__exit__.side_effect = close
            return wrapper

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / 'song.mixjam'
            target.write_text('earlier', encoding='utf-8')
            with mock.patch.object(mixjam, 'NamedTemporaryFile', side_effect=fail_close):
                with self.assertRaisesRegex(OSError, 'flush failed'):
                    mixjam.atomic_write_text(target, 'replacement')
            self.assertEqual(target.read_text(encoding='utf-8'), 'earlier')
            self.assertEqual(list(root.glob('*.tmp')), [])

    def test_song_save_keeps_both_earlier_files_when_the_second_replace_fails(self):
        replace = mixjam.os.replace
        for existing in (False, True):
            with self.subTest(existing=existing), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'song.mixjam'
                metadata = path.with_suffix('.meta.json')
                if existing:
                    mixjam.Song('earlier', key='C major').save(path)
                before = {file.name: file.read_bytes() for file in path.parent.iterdir()}
                destinations = []

                def fail_second(source, destination):
                    destinations.append(destination)
                    if len(destinations) == 2:
                        raise PermissionError('destination is locked')
                    replace(source, destination)

                with mock.patch.object(mixjam.os, 'replace', side_effect=fail_second):
                    with self.assertRaisesRegex(PermissionError, 'destination is locked'):
                        mixjam.Song('replacement', key='A minor').save(path)
                self.assertCountEqual(destinations[:2], [path, metadata])
                after = {file.name: file.read_bytes() for file in path.parent.iterdir()}
                self.assertEqual(after, before)

    def test_song_save_stages_metadata_before_changing_the_song(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'song.mixjam'
            mixjam.Song('earlier').save(path)
            before = {file.name: file.read_bytes() for file in path.parent.iterdir()}
            replacement = mixjam.Song('replacement', key=object())
            with self.assertRaises(TypeError):
                replacement.save(path)
            after = {file.name: file.read_bytes() for file in path.parent.iterdir()}
            self.assertEqual(after, before)

    def test_interrupted_song_save_keeps_the_files_in_agreement(self):
        replace = mixjam.os.replace
        for existing in (False, True):
            for step in ('metadata', 'song'):
                for renamed in (False, True):
                    with self.subTest(existing=existing, step=step, renamed=renamed), \
                            tempfile.TemporaryDirectory() as directory:
                        root = Path(directory)
                        path = root / 'song.mixjam'
                        metadata = path.with_suffix('.meta.json')
                        if existing:
                            mixjam.Song('earlier', key='C major').save(path)
                        before = {file.name: file.read_bytes() for file in root.iterdir()}
                        interrupted_path = metadata if step == 'metadata' else path
                        interrupted = False

                        def interrupt(source, destination):
                            nonlocal interrupted
                            if destination != interrupted_path or interrupted:
                                return replace(source, destination)
                            interrupted = True
                            if renamed:
                                replace(source, destination)
                            raise KeyboardInterrupt('cancelled save')

                        with mock.patch.object(mixjam.os, 'replace', side_effect=interrupt):
                            with self.assertRaisesRegex(KeyboardInterrupt, 'cancelled save'):
                                mixjam.Song('replacement', key='A minor').save(path)
                        if step == 'song' and renamed:
                            self.assertEqual(json.loads(path.read_text(encoding='utf-8'))['song']['title'],
                                             'replacement')
                            self.assertEqual(json.loads(metadata.read_text(encoding='utf-8'))['key'], 'A minor')
                        else:
                            after = {file.name: file.read_bytes() for file in root.iterdir()}
                            self.assertEqual(after, before)
                        self.assertEqual(list(root.glob('*.tmp')), [])

    def test_song_save_keeps_the_metadata_backup_if_restoration_fails(self):
        replace = mixjam.os.replace
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / 'song.mixjam'
            metadata = path.with_suffix('.meta.json')
            mixjam.Song('earlier', key='C major').save(path)
            previous_song, previous_metadata = path.read_bytes(), metadata.read_bytes()
            replaced_metadata = False

            def replace_metadata_once(source, destination):
                nonlocal replaced_metadata
                if destination != metadata or replaced_metadata:
                    raise PermissionError('destination is locked')
                replace(source, destination)
                replaced_metadata = True

            with mock.patch.object(mixjam.os, 'replace', side_effect=replace_metadata_once):
                with self.assertRaisesRegex(OSError, 'The earlier metadata is in') as failure:
                    mixjam.Song('replacement', key='A minor').save(path)
            backup, = root.glob('*.tmp')
            self.assertIn(str(backup), str(failure.exception))
            self.assertEqual(backup.read_bytes(), previous_metadata)
            self.assertEqual(path.read_bytes(), previous_song)


class MidiTests(unittest.TestCase):
    def read(self, events, **kwargs):
        with tempfile.TemporaryDirectory() as directory:
            return midinotes.notes(midi_file(Path(directory) / 'loop.mid', events, **kwargs))

    def test_a_note_without_a_note_off_is_held_to_the_end_of_the_track(self):
        events = b'\x00\x90\x3c\x64' + b'\x60\x90\x40\x64'  # C4 at 0, E4 at 96; no note-off
        found, ppq = self.read(events)
        self.assertEqual(ppq, 96)
        self.assertEqual(sorted(found), [(0, 96, 60), (96, 96, 64)])

    def test_a_retriggered_note_ends_the_earlier_one(self):
        events = b'\x00\x90\x3c\x64' + b'\x30\x90\x3c\x64' + b'\x30\x80\x3c\x00'
        found, _ = self.read(events)
        self.assertEqual(sorted(found), [(0, 48, 60), (48, 96, 60)])

    def test_note_on_with_zero_velocity_is_a_note_off(self):
        found, _ = self.read(b'\x00\x90\x3c\x64' + b'\x60\x90\x3c\x00')
        self.assertEqual(found, [(0, 96, 60)])

    def test_smpte_time_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = midi_file(Path(directory) / 'smpte.mid', b'', division=0xE728)
            with self.assertRaises(ValueError) as caught:
                midinotes.notes(path)
        self.assertIn('SMPTE', str(caught.exception))

    def test_per_bar_counts_held_beats_across_a_bar_line(self):
        bars = midinotes.per_bar([(0, 96 * 6, 60)], 96)
        self.assertEqual(dict(bars[0]), {60: 4.0})
        self.assertEqual(dict(bars[1]), {60: 2.0})

    def test_midi_loops_follow_loop_numbers_and_skip_files_without_a_key(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory) / 'Midi'
            folder.mkdir()
            for name in ('Lead 10 Am', 'Lead 2 Abm', 'Lead 3'):
                midi_file(folder / f'{name}.mid', b'')
            found = list(midichords.midi_loops(folder, 'A minor'))
        self.assertEqual([(path.stem, shift, key) for path, shift, key in found],
                         [('Lead 2 Abm', 1, 'Abm'), ('Lead 10 Am', 0, 'Am')])


class HarmonyShiftTests(unittest.TestCase):
    def test_shift_to_takes_key_names_pairs_and_pitch_classes(self):
        self.assertEqual(harmony.shift_to('Ab minor', 'A minor'), 1)
        self.assertEqual(harmony.shift_to(8, 9), 1)
        self.assertEqual(harmony.shift_to((8, 'minor'), (9, 'minor')), 1)
        self.assertEqual(harmony.shift_to((10, 'major'), 'C major'), 2)
        self.assertEqual(harmony.shift_to(0, 6), -6)
        self.assertEqual(harmony.shift_to('C', 'C'), 0)


class ImportAndFitCliTests(unittest.TestCase):
    def test_mixjam_imports_without_song_folder_on_sys_path(self):
        mixjam_path = Path(__file__).resolve().parents[1] / 'song' / 'mixjam.py'
        code = (
            'import importlib.util; '
            f'spec = importlib.util.spec_from_file_location("mixjam", {str(mixjam_path)!r}); '
            'mod = importlib.util.module_from_spec(spec); '
            'spec.loader.exec_module(mod)'
        )
        completed = subprocess.run(
            [sys.executable, '-B', '-I', '-c', code],
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_fit_reports_missing_progression_value_before_other_flags_and_rejects_empty(self):
        for argv in (
            ['fit.py', '--progression', '--key', 'A minor', 'folder1', 'folder2'],
            ['fit.py', '--progression', '', 'folder1'],
        ):
            with self.subTest(argv=argv), mock.patch.object(sys, 'argv', argv):
                with self.assertRaises(SystemExit) as caught:
                    fit.main()
                self.assertIn('--progression needs a value.', str(caught.exception))


if __name__ == '__main__':
    unittest.main()

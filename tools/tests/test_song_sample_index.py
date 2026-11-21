"""Checks for the song tools' sample index: names, WAV headers, and pack metadata."""
import io
import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import wave
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import samples  # noqa: E402


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
        self.assertEqual(
            [samples.parse_key(k) for k in ('A min', 'f#m', 'Gb', 'Bbm', 'F♯m', 'B♭', 'A   MAJ', 'E#', 'Cb', 'A-m', 'x')],
            ['Am', 'F#m', 'F#', 'A#m', 'F#m', 'A#', 'A', 'F', 'B', None, None],
        )
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

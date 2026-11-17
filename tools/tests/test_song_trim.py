"""Delivery trims preserve PCM data and files that share storage."""
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock
import wave

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import trim  # noqa: E402


class TrimTests(unittest.TestCase):
    def run_trim(self, source, destination, *options):
        with mock.patch.object(sys, 'argv', ['trim.py', str(source), str(destination), *options]), \
                mock.patch.object(sys, 'stdout', io.StringIO()):
            trim.main()

    def source(self, root, width=2):
        path = root / 'source.wav'
        params = wave._wave_params(2, width, 100, 50, 'NONE', 'not compressed')
        data = np.zeros((50, 2))
        data[:10] = np.array([1234, -2345]) * (1 << (8 * (width - 2)))
        trim.write(path, params, data)
        return path, params, data

    def test_tail_starts_after_last_loud_frame_and_ends_at_zero(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, _, original = self.source(root)
            out = root / 'delivery.wav'
            for tail, expected_frames in [('0', 10), ('0.01', 11), ('0.25', 35)]:
                with self.subTest(tail=tail):
                    self.run_trim(source, out, '--tail', tail)
                    _, result, _ = trim.read(out)
                    self.assertEqual(len(result), expected_frames)
                    np.testing.assert_array_equal(result[:10], original[:10])
                    if float(tail):
                        np.testing.assert_array_equal(result[-1], [0, 0])

    def test_delivery_preserves_pcm_width_rate_channels_and_samples(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for width in (2, 3, 4):
                with self.subTest(width=width):
                    source, params, original = self.source(root, width)
                    out = root / 'delivery.wav'
                    self.run_trim(source, out, '--tail', '0')
                    actual, result, _ = trim.read(out)
                    self.assertEqual(actual[:3], params[:3])
                    np.testing.assert_array_equal(result, original[:10])

    def test_replacement_preserves_other_hard_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, _, _ = self.source(root)
            original = source.read_bytes()
            out = root / 'delivery.wav'
            os.link(source, out)
            self.run_trim(source, out)
            self.assertEqual(source.read_bytes(), original)
            self.assertNotEqual(out.read_bytes(), original)

    def test_failed_write_preserves_existing_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, params, data = self.source(root)
            original = source.read_bytes()
            with mock.patch.object(wave.Wave_write, 'writeframes', side_effect=OSError('disk full')):
                with self.assertRaises(OSError):
                    trim.write(source, params, data)
            self.assertEqual(source.read_bytes(), original)
            self.assertEqual(list(root.iterdir()), [source])

    def test_invalid_options_do_not_replace_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, _, _ = self.source(root)
            out = root / 'delivery.wav'
            out.write_bytes(b'keep')
            for options in [('--tail', '-1'), ('--tail', 'nan'), ('--floor', 'inf'), ('--unknown', '1')]:
                with self.subTest(options=options), mock.patch.object(sys, 'stderr', io.StringIO()):
                    out.write_bytes(b'keep')
                    with self.assertRaises(SystemExit):
                        self.run_trim(source, out, *options)
                    self.assertEqual(out.read_bytes(), b'keep')


if __name__ == '__main__':
    unittest.main()

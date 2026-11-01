"""Compare parallel audibility measurements with direct time-domain subtraction."""
import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import audible  # noqa: E402


def direct_cells(mono, rate):
    frame = int(rate * 0.186)
    count = len(mono) // frame
    windowed = mono[:count * frame].reshape(count, frame) * np.hanning(frame)
    power = np.abs(np.fft.rfft(windowed, axis=1)) ** 2
    frequencies = np.fft.rfftfreq(frame, 1 / rate)
    return np.column_stack([
        power[:, (frequencies >= low) & (frequencies < high)].sum(axis=1)
        for low, high in zip(audible.BANDS, audible.BANDS[1:])
    ])


class AudibilityTests(unittest.TestCase):
    def test_spectral_subtraction_matches_direct_rest_energy(self):
        rate = 44100
        time = np.arange(rate * 3) / rate
        rng = np.random.default_rng(314)
        part = (0.2 * np.sin(2 * np.pi * 440 * time)).astype(np.float32)
        rest = (0.1 * rng.standard_normal(len(time))).astype(np.float32)
        mix = part + rest
        whole_spectrum, frame = audible.spectra(mix, rate)
        part_spectrum, _ = audible.spectra(part, rate)
        measured = audible.band_energy(whole_spectrum - part_spectrum, frame, rate)
        expected = direct_cells(mix - part, rate)
        np.testing.assert_allclose(measured, expected, rtol=2e-6, atol=1e-6)

    def test_parallel_table_preserves_order_padding_and_silence(self):
        rate = 12000
        time = np.arange(rate * 4) / rate
        parts = {
            'lead': (0.2 * np.sin(2 * np.pi * 440 * time)).astype(np.float32),
            'quiet': (0.01 * np.sin(2 * np.pi * 443 * time[:rate])).astype(np.float32),
            'silent': np.zeros(len(time), np.float32),
        }
        mix = parts['lead'] + np.pad(parts['quiet'], (0, len(time) - rate))
        document = {'song': {'bpm': 120}, 'lanes': [
            {'placements': [{'startTick': 0, 'durationTicks': 512}]},
            {'placements': [{'startTick': 0, 'durationTicks': 128}]},
            {'placements': [{'startTick': 0, 'durationTicks': 512}]},
        ]}
        groups = {'silent': [3], 'quiet': [2], 'lead': [1]}
        serial = audible.audibility(document, groups, rate, mix, parts.__getitem__, jobs=1)
        parallel = audible.audibility(document, groups, rate, mix, parts.__getitem__, jobs=3)
        self.assertEqual(serial, parallel)
        self.assertEqual([line.split()[0] for line in parallel[1:]], ['lead', 'quiet', 'silent'])
        self.assertIn('0%  <- drowned', parallel[-1])


if __name__ == '__main__':
    unittest.main()

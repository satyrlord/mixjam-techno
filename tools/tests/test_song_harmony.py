"""Checks for the song tools' key, scale, chord, and harmony fit helpers."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import harmony  # noqa: E402
from fit import fits  # noqa: E402
from midichords import shift_for  # noqa: E402


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


if __name__ == '__main__':
    unittest.main()

"""Checks for the sample index's edge measures, verdicts and palettes (tools/song)."""
import json
import io
import hashlib
import os
import struct
import sys
import tempfile
import unittest
from types import SimpleNamespace
from pathlib import Path
from unittest import mock

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'song'))
import palette  # noqa: E402
import sampletags  # noqa: E402
import verdicts  # noqa: E402
import mixjam  # noqa: E402


def write_wav(path, frames, rate=44100):
    """16-bit stereo PCM from float frames (n x 2)."""
    data = np.clip(np.round(np.asarray(frames) * 32767), -32768, 32767).astype('<i2').tobytes()
    header = (b'RIFF' + struct.pack('<I', 36 + len(data)) + b'WAVEfmt '
              + struct.pack('<IHHIIHH', 16, 1, 2, rate, rate * 4, 4, 16) + b'data' + struct.pack('<I', len(data)))
    Path(path).write_bytes(header + data)


class StemTests(unittest.TestCase):
    def test_numbers_chords_tempo_tags_and_channel_marks_make_one_family(self):
        self.assertEqual(verdicts.stem('Deep Pad 6.wav'), 'deep pad')
        self.assertEqual(verdicts.stem('BASS002_140_A_SL1.wav'), 'bass')
        self.assertEqual(verdicts.stem('Pad Am.wav'), 'pad')
        self.assertEqual(verdicts.stem('Lead Loop 4 150 Abm.wav'), 'lead loop')
        self.assertEqual(verdicts.stem('voice-002-r.wav'), 'voice')

    def test_a_letter_before_the_number_names_a_variant_not_a_chord(self):
        self.assertEqual(verdicts.stem('Drone A 1.wav'), 'drone a')
        self.assertNotEqual(verdicts.stem('Drone A 1.wav'), verdicts.stem('Drone B 1.wav'))


class HarmonyTests(unittest.TestCase):
    @staticmethod
    def chroma(*notes):
        bins = [0] * 12
        for note, weight in notes:
            bins[note] = weight
        return ','.join(map(str, bins))

    def test_a_chord_file_on_its_chord_fits_fully(self):
        row = dict(chroma=self.chroma((2, 400), (5, 300), (9, 300)))  # D F A
        self.assertAlmostEqual(verdicts.chord_fit(row, 'Dm'), 1.0)

    def test_a_chord_file_on_another_chord_fits_poorly(self):
        row = dict(chroma=self.chroma((0, 400), (4, 300), (7, 300)))  # C E G under a Dm label
        self.assertLess(verdicts.chord_fit(row, 'Dm'), 0.4)

    def test_a_loop_on_the_keys_fourth_belongs_to_the_key(self):
        self.assertTrue(verdicts.in_scale('Am', 'Dm'))
        self.assertTrue(verdicts.in_scale('C', 'G'))
        self.assertFalse(verdicts.in_scale('Am', 'F#m'))


class EdgeTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def test_a_stray_last_frame_is_an_end_spike(self):
        frames = np.zeros((44100, 2))
        frames[:22050] = 0.2 * np.sin(np.arange(22050) / 10)[:, None]
        frames[-1] = 0.28
        write_wav(self.root / 'spike.wav', frames)
        m = sampletags.edges(self.root / 'spike.wav')
        m['rms_db'] = -20.0
        self.assertIn('end-spike', sampletags.edge_tags(m, loop=False))

    def test_a_loop_that_wraps_mid_wave_clicks_and_one_that_wraps_smoothly_does_not(self):
        t = np.arange(44100 * 2)
        smooth = 0.3 * np.sin(2 * np.pi * 441 * t / 44100)  # whole cycles: the wrap continues the wave
        broken = 0.3 * np.sin(2 * np.pi * 441 * t / 44100 + 1.5)
        write_wav(self.root / 'smooth.wav', np.column_stack([smooth, smooth]))
        write_wav(self.root / 'broken.wav', np.column_stack([broken[:-37], broken[:-37]]))
        for name, expected in (('smooth.wav', False), ('broken.wav', True)):
            m = sampletags.edges(self.root / name)
            m['rms_db'] = -13.0
            self.assertEqual('seam-click' in sampletags.edge_tags(m, loop=True), expected, name)

    def test_antiphase_stereo_is_not_an_end_spike_but_a_single_channel_spike_is(self):
        t = np.arange(44100)
        left = 0.4 * np.sin(2 * np.pi * 441 * t / 44100)
        frames = np.column_stack([left, -0.98 * left])
        write_wav(self.root / 'continuous.wav', frames)
        continuous = sampletags.edges(self.root / 'continuous.wav')
        continuous['rms_db'] = -50
        self.assertNotIn('end-spike', sampletags.edge_tags(continuous, loop=False))
        frames[-441:] = 0
        frames[-1, 1] = 0.4
        write_wav(self.root / 'spike-one-channel.wav', frames)
        spike = sampletags.edges(self.root / 'spike-one-channel.wav')
        spike['rms_db'] = -50
        self.assertIn('end-spike', sampletags.edge_tags(spike, loop=False))

    def test_exact_antiphase_stereo_has_audible_level_and_cut_end(self):
        t = np.arange(44100)
        wave = 0.3 * np.sin(2 * np.pi * 441 * t / 44100)
        write_wav(self.root / 'antiphase.wav', np.column_stack([wave, -wave]))
        result = sampletags.row_for((str(self.root), 'antiphase.wav', '1', '1', False))
        self.assertFalse(result.get('error'), result)
        self.assertGreater(float(result['rms_db']), -20)
        self.assertGreater(float(result['end_db']), -20)
        self.assertIn('cut-end', result['tags'].split())
        self.assertNotIn('quiet', result['tags'].split())

    def test_older_failed_antiphase_measurement_is_retried(self):
        t = np.arange(44100)
        wave = 0.3 * np.sin(2 * np.pi * 441 * t / 44100)
        write_wav(self.root / 'antiphase.wav', np.column_stack([wave, -wave]))
        target = self.root / 'tags.tsv'
        row = dict(root=str(self.root), path='antiphase.wav', bytes='1', mtime='1', type='wav', kind='pad', bars='')
        old = dict(root=row['root'], path=row['path'], bytes='1', mtime='1', error='a measurement is not a number')
        sampletags.save([old], target)
        with mock.patch.object(sampletags, 'tags_path', return_value=target), \
             mock.patch.object(sampletags.samples, 'load', return_value=[row]), \
             mock.patch.object(sampletags, 'ProcessPoolExecutor', InlinePool):
            sampletags.tag(1)
        result = sampletags.load(target)[0]
        self.assertEqual(result['error'], '')
        self.assertIn('cut-end', result['tags'].split())

    def test_cached_edge_tags_follow_new_loop_metadata(self):
        t = np.arange(44100 * 2 - 37)
        wave = 0.3 * np.sin(2 * np.pi * 441 * t / 44100 + 1.5)
        write_wav(self.root / 'sound.wav', np.column_stack([wave, wave]))
        target = self.root / 'tags.tsv'
        row = dict(root=str(self.root), path='sound.wav', bytes='1', mtime='1', type='wav', kind='pad', bars='')
        with mock.patch.object(sampletags, 'tags_path', return_value=target), \
             mock.patch.object(sampletags.samples, 'load', return_value=[row]), \
             mock.patch.object(sampletags, 'ProcessPoolExecutor', InlinePool):
            sampletags.tag(1)
            self.assertIn('cut-end', sampletags.load(target)[0]['tags'].split())
            row['bars'] = '1'
            sampletags.tag(1)
            tags = sampletags.load(target)[0]['tags'].split()
            self.assertIn('seam-click', tags)
            self.assertNotIn('cut-end', tags)

    def test_failed_edge_upgrade_retries_without_remeasuring_the_rest(self):
        t = np.arange(44100)
        wave = 0.3 * np.sin(2 * np.pi * 441 * t / 44100)
        write_wav(self.root / 'sound.wav', np.column_stack([wave, wave]))
        target = self.root / 'tags.tsv'
        row = dict(root=str(self.root), path='sound.wav', bytes='1', mtime='1', type='wav', kind='pad', bars='1')
        with mock.patch.object(sampletags, 'tags_path', return_value=target), \
             mock.patch.object(sampletags.samples, 'load', return_value=[row]), \
             mock.patch.object(sampletags, 'ProcessPoolExecutor', InlinePool):
            sampletags.tag(1)
            stored = sampletags.load(target)[0]
            stored['error'] = 'edges: file temporarily unavailable'
            stored['seam_step'] = ''
            sampletags.save([stored], target)
            with mock.patch.object(sampletags, 'measure', side_effect=AssertionError('must not remeasure')):
                sampletags.tag(1)
            stored = sampletags.load(target)[0]
            self.assertEqual(stored['error'], '')
            self.assertTrue(stored['seam_step'])

    def test_new_measurement_preserves_non_edge_data_when_edge_read_fails(self):
        t = np.arange(44100)
        wave = 0.3 * np.sin(2 * np.pi * 441 * t / 44100)
        write_wav(self.root / 'sound.wav', np.column_stack([wave, wave]))
        target = self.root / 'tags.tsv'
        row = dict(root=str(self.root), path='sound.wav', bytes='1', mtime='1', type='wav', kind='pad', bars='1')
        with mock.patch.object(sampletags, 'tags_path', return_value=target), \
             mock.patch.object(sampletags.samples, 'load', return_value=[row]), \
             mock.patch.object(sampletags, 'ProcessPoolExecutor', InlinePool):
            with mock.patch.object(sampletags, 'edges', side_effect=OSError('temporary error')):
                sampletags.tag(1)
            stored = sampletags.load(target)[0]
            self.assertTrue(stored['error'].startswith('edges:'))
            self.assertTrue(stored['rms_db'])
            self.assertTrue(stored['tags'])
            with mock.patch.object(sampletags, 'measure', side_effect=AssertionError('must not remeasure')):
                sampletags.tag(1)
            self.assertEqual(sampletags.load(target)[0]['error'], '')

    def test_failed_edge_read_drops_the_edge_tags_of_an_earlier_algorithm(self):
        target = self.root / 'tags.tsv'
        row = dict(root=str(self.root), path='gone.wav', bytes='1', mtime='1', type='wav', kind='pad', bars='1',
                   chord='', key='', key_from='')
        old = dict(root=row['root'], path=row['path'], bytes='1', mtime='1', measure_version=sampletags.MEASURE_VERSION,
                   rms_db='-20', peak_db='-6', flatness='0.5', key='', key_fit='0', tuning='0', chroma='1',
                   end_db='-20', end_spike='100', end_abs='0.5', seam_step='1', seam_abs='0.1', edge_version='2',
                   tags='bright end-spike', error='')
        sampletags.save([old], target)
        with mock.patch.object(sampletags, 'tags_path', return_value=target), \
             mock.patch.object(sampletags.samples, 'load', return_value=[row]), \
             mock.patch.object(sampletags, 'ProcessPoolExecutor', InlinePool):
            sampletags.tag(1)  # gone.wav does not exist, so its edges cannot be read again
        stored = sampletags.load(target)[0]
        self.assertTrue(stored['error'].startswith('edges:'))
        self.assertEqual(stored['tags'], 'bright')
        output = self.root / 'verdicts.tsv'
        with mock.patch.object(verdicts.samples, 'load', return_value=[row]), \
             mock.patch.object(verdicts.sampletags, 'load', return_value=[stored]), \
             mock.patch.object(verdicts, 'load_owner', return_value=[]), \
             mock.patch.object(verdicts, 'verdicts_path', return_value=output):
            verdicts.build()
            result = verdicts.load(output)[0]
        self.assertEqual((result['verdict'], result['reasons']), ('ok', ''))

    def test_old_edge_algorithm_is_remeasured(self):
        t = np.arange(44100)
        wave = 0.4 * np.sin(2 * np.pi * 441 * t / 44100)
        write_wav(self.root / 'sound.wav', np.column_stack([wave, -0.98 * wave]))
        target = self.root / 'tags.tsv'
        row = dict(root=str(self.root), path='sound.wav', bytes='1', mtime='1', type='wav', kind='pad', bars='1')
        with mock.patch.object(sampletags, 'tags_path', return_value=target), \
             mock.patch.object(sampletags.samples, 'load', return_value=[row]), \
             mock.patch.object(sampletags, 'ProcessPoolExecutor', InlinePool):
            sampletags.tag(1)
            stored = sampletags.load(target)[0]
            stored['edge_version'] = ''
            stored['end_spike'] = '100'
            stored['tags'] += ' end-spike'
            sampletags.save([stored], target)
            sampletags.tag(1)
            stored = sampletags.load(target)[0]
            self.assertNotIn('end-spike', stored['tags'].split())
            self.assertLess(float(stored['end_spike']), 10)


class InlinePool:
    def __init__(self, _jobs):
        pass

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        pass

    def map(self, function, items, chunksize):
        return map(function, items)


class OwnerVerdictTests(unittest.TestCase):
    def test_the_last_matching_line_wins_and_case_does_not_count(self):
        rules = [dict(match='e:/lib/pads/*', verdict='approved'), dict(match='e:/lib/pads/pad 2.wav', verdict='rejected')]
        self.assertEqual(verdicts.owner_verdict(rules, 'E:\\lib', 'Pads/Pad 1.wav')['verdict'], 'approved')
        self.assertEqual(verdicts.owner_verdict(rules, 'E:\\lib', 'Pads/Pad 2.wav')['verdict'], 'rejected')
        self.assertIsNone(verdicts.owner_verdict(rules, 'E:\\lib', 'Bass/Bass 1.wav'))

    def test_brackets_are_literal_and_only_star_is_a_wildcard(self):
        rules = [dict(match='e:/lib/[rare]/pad*.wav', verdict='rejected')]
        self.assertIsNotNone(verdicts.owner_verdict(rules, 'E:\\lib', '[Rare]/Pad 1.wav'))
        self.assertIsNone(verdicts.owner_verdict(rules, 'E:\\lib', 'r/Pad 1.wav'))

    def test_stale_measurements_are_not_used_for_new_audio(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / 'verdicts.tsv'
            info = dict(root='E:/lib', path='pad.wav', type='wav', kind='pad', chord='', key='',
                        key_from='', bars='', bytes='2', mtime='2')
            measured = dict(root='E:/lib', path='pad.wav', bytes='1', mtime='1', error='', tags='quiet',
                            flatness='0.01', key_fit='0.9', tuning='30', rms_db='-20', peak_db='-6', chroma='1')
            with mock.patch.object(verdicts.samples, 'load', return_value=[info]), \
                 mock.patch.object(verdicts.sampletags, 'load', return_value=[measured]), \
                 mock.patch.object(verdicts, 'load_owner', return_value=[]), \
                 mock.patch.object(verdicts, 'verdicts_path', return_value=target):
                verdicts.build()
                result = verdicts.load(target)[0]
            self.assertEqual(result['verdict'], 'reject')
            self.assertIn('stale-measurement', result['reasons'])
            self.assertEqual(result['tune'], '')
            self.assertNotIn('quiet', result['reasons'])

    def test_family_tuning_counts_a_member_whose_only_error_is_its_edges(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / 'verdicts.tsv'
            index, measured = [], []
            for number, error in ((1, ''), (2, 'edges: temporarily unavailable'), (3, 'edges: temporarily unavailable')):
                path = f'Pads/Pad {number}.wav'
                index.append(dict(root='E:/lib', path=path, type='wav', kind='pad', chord='', key='', key_from='',
                                  bars='', bytes='1', mtime='1'))
                measured.append(dict(root='E:/lib', path=path, bytes='1', mtime='1',
                                     measure_version=sampletags.MEASURE_VERSION, error=error, tags='',
                                     flatness='0.01', key_fit='0.9', tuning='30' if error else '0',
                                     rms_db='-20', peak_db='-6', chroma=str(number)))
            with mock.patch.object(verdicts.samples, 'load', return_value=index), \
                 mock.patch.object(verdicts.sampletags, 'load', return_value=measured), \
                 mock.patch.object(verdicts, 'load_owner', return_value=[]), \
                 mock.patch.object(verdicts, 'verdicts_path', return_value=target):
                verdicts.build()
                rows = verdicts.load(target)
        # The median of 0, 30 and 30 cents: the two members with only an edge error count.
        self.assertEqual({row['tune'] for row in rows}, {'-30'})
        self.assertTrue(all('family-off-tune' in row['reasons'].split() for row in rows))


class PaletteTests(unittest.TestCase):
    def test_a_family_admits_its_members_but_never_a_file_the_owner_rejected(self):
        roots = ['E:\\lib']
        family = verdicts.family_of('E:\\lib', 'Pads/Pad 1.wav')
        chosen = dict(families=[family], files=[])
        rules = [dict(match='e:/lib/pads/pad 3.wav', verdict='rejected', note='', date='')]
        with mock.patch.object(palette, '_owner_rules', rules):
            self.assertTrue(palette.allowed(chosen, 'E:\\lib\\Pads\\Pad 2.wav', roots))
            self.assertFalse(palette.allowed(chosen, 'E:\\lib\\Pads\\Pad 3.wav', roots))
            self.assertFalse(palette.allowed(chosen, 'E:\\lib\\Bass\\Bass 1.wav', roots))

    @staticmethod
    def catalog_release(name, product, samples):
        """A format-3 catalog: sources by size and SHA-256 only, names as label keys."""
        sha = hashlib.sha256(name.encode()).hexdigest()
        fingerprint = hashlib.sha256(sha.encode() + struct.pack('<Q', 1) + b'\n').hexdigest()
        return dict(schema=3, product=product, release=name, fingerprint=fingerprint,
                    sources=[dict(bytes=1, sha256=sha)], samples=samples)

    @staticmethod
    def catalog_sample(sample_id, digest='11', code=None):
        return dict(native_id=sample_id, code=code or f'pad{sample_id}.snd', label_key='0123456789abcdef',
                    group_name='Layer', bars=1, source_bpm=140, frame_count=100, pcm_digest=digest * 16)

    def test_product_sounds_resolve_through_format_three_releases(self):
        with tempfile.TemporaryDirectory() as folder, tempfile.TemporaryDirectory() as names:
            def release(name, sample_id):
                return self.catalog_release(name, 'd4', [self.catalog_sample(sample_id)])
            Path(folder, 'd4-cd.json').write_text(json.dumps(release('cd', 7)))
            Path(folder, 'd4-download.json').write_text(json.dumps(release('download', 8)))
            conflict = release('other', 7)
            conflict['samples'][0]['pcm_digest'] = '22' * 16
            conflict['samples'].append(dict(conflict['samples'][0], native_id=9, code='pad9'))
            Path(folder, 'd4-other.json').write_text(json.dumps(conflict))
            Path(folder, 'd4-bad.json').write_text(json.dumps(dict(release('bad', 10), fingerprint='wrong')))
            Path(folder, 'd4-malformed.json').write_text(json.dumps(dict(release('malformed', 11), release=123)))
            named = release('named', 12)
            named['samples'][0]['name'] = 'deep pad 12'
            Path(folder, 'd4-named.json').write_text(json.dumps(named))
            duplicated = json.dumps(release('duplicate', 13)).replace('"product": "d4"',
                                                                        '"product": "d4", "product": "d4"')
            Path(folder, 'd4-duplicate.json').write_text(duplicated)
            old = release('old', 14)
            old['schema'] = 2
            Path(folder, 'd4-old.json').write_text(json.dumps(old))
            # Names live in the generator's private files, not in the catalogs.
            Path(names, 'd4-cd.json').write_text(json.dumps(dict(product='d4', release='cd',
                                                                 names=[[7, 'deep pad 7']])))
            with mock.patch.dict(os.environ, {'SAMPLE_CATALOGS': folder, 'SAMPLE_CATALOG_NAMES': names}), \
                    mock.patch.object(palette, '_catalog', None):
                catalog = palette.product_catalog()
                self.assertEqual(catalog[5 * 100_000 + 7], ('d4', 'Layer', 'deep pad 7'))
                self.assertEqual(catalog[5 * 100_000 + 8], ('d4', 'Layer', 'pad8'))
                for missing in (9, 10, 11, 12, 13, 14):
                    self.assertNotIn(5 * 100_000 + missing, catalog)

    def test_conflicting_releases_load_in_the_applications_file_order(self):
        with tempfile.TemporaryDirectory() as folder, tempfile.TemporaryDirectory() as names:
            def release(name, product, digest):
                return self.catalog_release(name, product, [self.catalog_sample(7, digest, f'kit\\{name}7.snd')])
            # The application sorts file names with case, so D4-b.json loads before d4-a.json. The earlier
            # product slug of D4 also names the product.
            Path(folder, 'd4-a.json').write_text(json.dumps(release('lower', 'd4', '11')))
            Path(folder, 'D4-b.json').write_text(json.dumps(release('upper', '\x64ance4', '22')))
            with mock.patch.dict(os.environ, {'SAMPLE_CATALOGS': folder, 'SAMPLE_CATALOG_NAMES': names}), \
                    mock.patch.object(palette, '_catalog', None):
                self.assertEqual(palette.product_catalog()[5 * 100_000 + 7], ('d4', 'Layer', 'upper7'))

    def test_product_catalog_names_come_from_the_accepted_release(self):
        with tempfile.TemporaryDirectory() as folder, tempfile.TemporaryDirectory() as names:
            current = self.catalog_release('current', 'd4', [self.catalog_sample(7)])
            rejected = self.catalog_release('rejected', 'd4', [self.catalog_sample(7, digest='22')])
            Path(folder, 'd4-current.json').write_text(json.dumps(current))
            Path(folder, 'd4-rejected.json').write_text(json.dumps(rejected))
            # An absent or rejected release's names must not name an accepted release's sounds.
            for release in ('absent', 'rejected'):
                Path(names, f'0-{release}.json').write_text(json.dumps(dict(
                    product='d4', release=release, names=[[7, f'Wrong {release} sound']])))
            with mock.patch.dict(os.environ, {'SAMPLE_CATALOGS': folder, 'SAMPLE_CATALOG_NAMES': names}):
                with mock.patch.object(palette, '_catalog', None):
                    self.assertEqual(palette.product_catalog()[500007], ('d4', 'Layer', 'pad7'))
                Path(names, 'd4-current.json').write_text(json.dumps(dict(
                    product='d4', release='current', names=[[7, 'Correct sound']])))
                with mock.patch.object(palette, '_catalog', None):
                    self.assertEqual(palette.product_catalog()[500007], ('d4', 'Layer', 'Correct sound'))

    def test_invalid_catalog_name_files_do_not_partially_load_or_break_song_reports(self):
        with tempfile.TemporaryDirectory() as folder, tempfile.TemporaryDirectory() as names:
            catalog = self.catalog_release('current', 'd4', [self.catalog_sample(7), self.catalog_sample(8)])
            Path(folder, 'd4-current.json').write_text(json.dumps(catalog))
            song = Path(folder, 'song.mixjam')
            song.write_text(json.dumps(dict(samples=[], lanes=[dict(placements=[dict(sampleId=500007)])])))
            bad_entries = ([[7, 42]], [[7, '']], [[True, 'wrong']], [[7, 'wrong'], [8, 42]],
                           [[7, 'first'], [7, 'duplicate']], [[7, 'wrong'], [8]])
            for entries in bad_entries:
                with self.subTest(entries=entries):
                    Path(names, 'd4-current.json').write_text(json.dumps(dict(
                        product='d4', release='current', names=entries)))
                    errors = io.StringIO()
                    with mock.patch.dict(os.environ, {'SAMPLE_CATALOGS': folder, 'SAMPLE_CATALOG_NAMES': names}), \
                            mock.patch.object(palette, '_catalog', None), \
                            mock.patch.object(palette, 'roots', return_value=[]), mock.patch('sys.stderr', errors):
                        placed = palette.placed_samples(song)
                    self.assertEqual(placed[0]['name'], 'pad7')
                    self.assertIn('catalog names were not loaded', errors.getvalue())

    def test_make_and_check_round_trip_a_pathless_native_sound(self):
        with tempfile.TemporaryDirectory() as folder:
            song, chosen = Path(folder) / 'song.mixjam', Path(folder) / 'palette.json'
            song.write_text(json.dumps(dict(samples=[], lanes=[dict(placements=[dict(sampleId=500007)])])))
            args = SimpleNamespace(title='test', palette=str(chosen), source=str(song))
            with mock.patch.object(palette, 'roots', return_value=[]), \
                 mock.patch.object(palette.verdicts, 'load', return_value=[]), \
                 mock.patch.object(palette, 'product_catalog', return_value={500007: ('d4', 'Layer', 'Pad 1')}):
                palette.make(args)
                self.assertEqual(palette.check(song, chosen), 0)

    def test_external_file_rejected_by_owner_is_outside_a_palette(self):
        rules = [dict(match='e:/elsewhere/forbidden.wav', verdict='rejected')]
        with mock.patch.object(palette, '_owner_rules', rules):
            self.assertFalse(palette.allowed(dict(families=[], files=['E:/elsewhere/forbidden.wav']),
                                             'E:/elsewhere/forbidden.wav', ['E:/lib']))

    def test_owner_rule_matches_normalized_windows_dot_segments(self):
        rules = [dict(match='e:/elsewhere/forbidden.wav', verdict='rejected')]
        chosen = dict(families=[], files=['E:/elsewhere/forbidden.wav'])
        with mock.patch.object(palette, '_owner_rules', rules):
            self.assertFalse(palette.allowed(chosen, 'E:/lib/../ELSEWHERE/forbidden.wav', ['E:/lib']))
            self.assertFalse(palette.allowed(chosen, 'e:\\lib\\..\\elsewhere\\.\\forbidden.wav', ['E:/lib']))

    def test_verbatim_windows_paths_match_drive_and_unc_families_files_and_owner_rules(self):
        pairs = [
            ('E:\\lib', '\\\\?\\E:\\LIB'),
            ('\\\\server\\share\\lib', '\\\\?\\UNC\\SERVER\\SHARE\\LIB'),
        ]
        for ordinary, verbatim in pairs:
            family = verdicts.family_of(ordinary, 'Pads/Pad 1.wav')
            chosen = dict(families=[family], files=[ordinary + '\\Single.wav'])
            rule = dict(match=(ordinary + '\\Pads\\Pad 3.wav').replace('\\', '/').lower(), verdict='rejected')
            with self.subTest(root=ordinary), mock.patch.object(palette, '_owner_rules', [rule]):
                self.assertTrue(palette.allowed(chosen, verbatim + '\\Pads\\Pad 2.wav', [ordinary]))
                self.assertTrue(palette.allowed(chosen, ordinary + '\\Pads\\Pad 2.wav', [verbatim]))
                self.assertTrue(palette.allowed(chosen, verbatim + '\\Single.wav', [ordinary]))
                self.assertFalse(palette.allowed(chosen, verbatim + '\\Pads\\Pad 3.wav', [ordinary]))

    def test_windows_family_paths_match_independent_of_case(self):
        family = verdicts.family_of('E:\\lib', 'Pads/Pad 1.wav')
        with mock.patch.object(palette, '_owner_rules', []):
            self.assertTrue(palette.allowed(dict(families=[family], files=[]),
                                            'e:\\LIB\\PADS\\PAD 2.wav', ['E:\\lib']))

    def test_family_suggestions_do_not_use_old_key_or_tags(self):
        family = verdicts.family_of('E:/lib', 'Pads/Pad 1.wav')
        info = dict(root='E:/lib', path='Pads/Pad 1.wav', bytes='2', mtime='2', bpm='140')
        measured = dict(root=info['root'], path=info['path'], bytes='1', mtime='1', key='Am', tags='bright')
        row = dict(root=info['root'], path=info['path'], family=family, verdict='approved', kind='pad', tune='+10',
                   bytes='1', mtime='1')
        with mock.patch.object(palette.samples, 'load', return_value=[info]), \
             mock.patch.object(palette.sampletags, 'load', return_value=[measured]), \
             mock.patch.object(palette.verdicts, 'load', return_value=[row]):
            summary = palette.family_table()[family]
        self.assertEqual(summary['key'], '')
        self.assertEqual(summary['tags'], set())
        self.assertEqual(summary['verdicts']['approved'], 0)
        self.assertEqual(summary['verdicts']['reject'], 1)
        self.assertEqual(summary['tune'], '')

    def test_enabling_a_palette_checks_already_linked_sounds(self):
        with tempfile.TemporaryDirectory() as folder:
            sound = Path(folder) / 'sound.wav'
            write_wav(sound, np.column_stack([np.ones(441), np.ones(441)]) * 0.1)
            chosen = Path(folder) / 'palette.json'
            chosen.write_text(json.dumps(dict(families=[], files=[])))
            song = mixjam.Song('test')
            song.sound(sound)
            with mock.patch.object(palette, 'roots', return_value=[]), mock.patch.object(palette, '_owner_rules', []):
                with self.assertRaisesRegex(ValueError, 'outside the palette'):
                    song.use_palette(chosen)
            self.assertIsNone(song.palette)


if __name__ == '__main__':
    unittest.main()

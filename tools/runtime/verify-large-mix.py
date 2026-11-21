"""Verify large MIX imports with a copied executable and retained local artifacts.

This checks file-size boundaries, preserved audio, saved projects, and a short
render with synthetic fixtures that need no original media. An optional real
MIX corpus and WAV provider add per-song conversion records with a separate
verdict. Artifacts (about 1 GB, including a 500 MB fixture) are retained in the
artifact folder unless --clean is given. It does not check physical playback.
"""
from pathlib import Path
import argparse
import copy
from datetime import datetime, timezone
import hashlib
import json
import os
import platform
import shutil
import struct
import subprocess
import tempfile
import time
import wave
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mixjam_song import read_song  # noqa: E402


LIMIT = 500_000_000
BLOCK = 1024 * 1024


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(BLOCK), b''):
            digest.update(block)
    return digest.hexdigest()


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def text_block(value):
    return struct.pack('<H', len(value)) + value


def mark_sparse(output):
    if os.name == 'nt':
        import ctypes
        from ctypes import wintypes
        import msvcrt
        control = ctypes.WinDLL('kernel32', use_last_error=True).DeviceIoControl
        control.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.LPVOID,
                            wintypes.DWORD, wintypes.LPVOID, wintypes.DWORD,
                            ctypes.POINTER(wintypes.DWORD), wintypes.LPVOID]
        control.restype = wintypes.BOOL
        returned = wintypes.DWORD()
        if not control(msvcrt.get_osfhandle(output.fileno()), 0x900C4,
                       None, 0, None, 0, ctypes.byref(returned), None):
            raise ctypes.WinError(ctypes.get_last_error())


def write_exact_fixture(path):
    """Write three distinct WAVs and padding in fixed-size blocks."""
    frames = 45_000_000
    pcm_bytes = frames * 2
    wave_bytes = pcm_bytes + 44
    expected = []
    paths = [f'C:\\large-mix-fixture\\clip-{i}.wav'.encode('ascii') for i in range(3)]
    timeline = bytearray()
    for index, source_path in enumerate(paths):
        timeline += struct.pack('<BIBiH', index + 1, 1, 1, -1, 0)
        timeline += text_block(source_path)
    trailer = struct.pack('<I', len(timeline) + 4) + timeline + bytes.fromhex('a97a8a01')
    with path.open('xb') as output:
        output.write(struct.pack('<IIHI', 0x2571, 0, 140, 140))
        output.write(text_block(b'Large MIX check\0#SKKENNUNG#:0000000\0\1'))
        output.write(text_block(b'Exact 500 MB fixture\0Mixer text'))
        # D3 saves 32 sample lanes plus the generated-audio mixer lane.
        output.write(struct.pack('<HHI', 128, 16384, 0) * 33)
        for index, source_path in enumerate(paths):
            output.write(text_block(source_path + b'\0\1'))
            output.write(struct.pack('<I', wave_bytes))
            header = struct.pack('<4sI4s4sIHHIIHH4sI', b'RIFF', pcm_bytes + 36,
                                 b'WAVE', b'fmt ', 16, 1, 1, 44100, 88200,
                                 2, 16, b'data', pcm_bytes)
            digest = hashlib.sha256(header)
            output.write(header)
            block = struct.pack('<h', 1000 + index) * (BLOCK // 2)
            remaining = pcm_bytes
            while remaining:
                piece = block[:min(len(block), remaining)]
                output.write(piece)
                digest.update(piece)
                remaining -= len(piece)
            expected.append({'source_path': source_path.decode('ascii'),
                             'bytes': wave_bytes, 'frames': frames,
                             'sha256': digest.hexdigest()})
        padding = LIMIT - output.tell() - len(trailer)
        require(padding >= 0, 'Synthetic fixture payload exceeds its target size.')
        zeroes = bytes(BLOCK)
        while padding:
            piece = zeroes[:min(BLOCK, padding)]
            output.write(piece)
            padding -= len(piece)
        output.write(trailer)
    require(path.stat().st_size == LIMIT, 'Synthetic fixture has the wrong byte size.')
    require(sum(item['bytes'] for item in expected) > 256 * 1024 * 1024,
            'Synthetic fixture does not exceed the former aggregate asset limit.')
    return expected


class Verifier:
    """Isolated verification harness for large MIX import and short-render limits."""

    def __init__(self, args):
        self.args = args
        self.exe_source = args.exe.resolve()
        self.report_path = args.report.resolve()
        self.fixtures = args.fixtures.resolve() if args.fixtures else None
        self.samples = args.samples.resolve() if args.samples else None
        if args.artifacts is None:
            self.root = Path(tempfile.mkdtemp(prefix='mixjam-techno-large-mix-'))
        else:
            self.root = args.artifacts.resolve()
            if self.root.exists():
                raise ValueError(f'Artifact folder already exists: {self.root}')
            if args.clean and self.root in self.report_path.parents:
                raise ValueError('--clean would delete a report inside the artifact folder.')
            self.root.mkdir(parents=True)
        self.env = os.environ.copy()
        # Host reference media and capture settings must not reach the copied executable.
        for name in [key for key in self.env if key.startswith('MIXJAM_TECHNO_')]:
            self.env.pop(name)
        self.env['MIXJAM_TECHNO_DATA_DIR'] = str(self.root / 'user-data')
        self.exe = self.root / 'MixJam-Techno.exe'
        self.started = time.monotonic()
        self.successful = []
        self.report = {
            'passed': False, 'started_utc': datetime.now(timezone.utc).isoformat(),
            'platform': platform.platform(), 'python': platform.python_version(),
            'exe_source': str(self.exe_source),
            'fixtures': str(self.fixtures) if self.fixtures else None,
            'samples': str(self.samples) if self.samples else None,
            'artifacts': str(self.root), 'data_directory': self.env['MIXJAM_TECHNO_DATA_DIR'],
            'limit_bytes': LIMIT,
            'scope': 'Copied executable, independent cwd, isolated user data, no host provider '
                     'environment. Synthetic exact-limit import contains three WAVs totaling over '
                     '256 MiB; the over-limit file is one byte larger. Optional real MIX files are '
                     'converted once with the explicit WAV provider, then the saved .mixjam song is reopened; '
                     'their verdict is separate. Render checks use one selected clip, moved to '
                     'zero, limited to 256 ticks.',
            'limitations': ['Host installations remain visible; no filesystem sandbox is used.',
                            'No full-song render, GUI, or physical audio check.',
                            'An over-limit file is created by seeking; filesystem allocation is not measured.',
                            'Real fixture results depend on the supplied WAV provider; they do not '
                            'change the size-limit verdict.'],
            'commands': [], 'checks': [], 'failures': [],
            'real_fixtures': [], 'real_fixture_failures': [], 'real_fixtures_passed': None,
        }

    def save_report(self):
        self.report['seconds'] = time.monotonic() - self.started
        self.report_path.parent.mkdir(parents=True, exist_ok=True)
        self.report_path.write_text(json.dumps(self.report, indent=2), encoding='utf-8')

    def run(self, *arguments, success=True):
        command = {'args': list(map(str, arguments))}
        self.report['commands'].append(command)
        command_start = time.monotonic()
        try:
            result = subprocess.run([str(self.exe), *command['args']], cwd=self.root, env=self.env,
                                    capture_output=True, text=True, errors='replace',
                                    timeout=self.args.timeout)
            command.update(exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
            require((result.returncode == 0) == success,
                    f'Unexpected exit {result.returncode}; see command {len(self.report["commands"]) - 1}.')
            return result
        except subprocess.TimeoutExpired as error:
            command['timeout'] = self.args.timeout
            for field in ('stdout', 'stderr'):
                value = getattr(error, field) or ''
                command[field] = value.decode(errors='replace') if isinstance(value, bytes) else value
            raise
        except OSError as error:
            command['error'] = str(error)
            raise
        finally:
            command['seconds'] = time.monotonic() - command_start
            self.save_report()

    def check(self, name, action, required=True):
        check_start = time.monotonic()
        item = {'name': name, 'passed': False, 'required': required}
        self.report['checks'].append(item)
        try:
            details = action()
            item.update(passed=True, details=details)
        except Exception as error:
            item['error'] = f'{type(error).__name__}: {error}'
            failures = self.report['failures'] if required else self.report['real_fixture_failures']
            failures.append({'check': name, 'error': item['error']})
        item['seconds'] = time.monotonic() - check_start
        self.save_report()
        print(json.dumps(item), flush=True)
        return item['passed']

    def convert_and_reopen(self, song, destination, provider=None):
        provider_args = ['--samples', provider] if provider else []
        self.run('convert', *provider_args, '--mix', song, '--output', destination)
        project = read_song(destination)
        reopened = destination.with_name(destination.stem + '-reopened.mixjam')
        self.run('convert', *provider_args, '--mix', destination, '--output', reopened)
        require(read_song(reopened) == project, 'Saved and reopened songs differ.')
        return project

    def render_short(self, project, label, provider=None):
        imported = {item['id'] for item in project['imported_samples']}
        clip = next((clip for clip in project['clips'] if clip['sample_id'] in imported), None)
        require(clip is not None, 'No imported-audio clip is available for the short render.')
        short = copy.deepcopy(project)
        clip = copy.deepcopy(clip)
        clip['start_tick'] = 0
        clip['length_ticks'] = min(clip['length_ticks'], 256)
        short['clips'] = [clip]
        short['loop_region'] = None
        short['imported_samples'] = [item for item in short['imported_samples']
                                     if item['id'] == clip['sample_id']]
        path = self.root / f'{label}-short.json'
        path.write_text(json.dumps(short), encoding='utf-8')
        output = self.root / f'{label}-short.wav'
        provider_args = ['--samples', provider] if provider else []
        self.run('render', *provider_args, '--mix', path, '--output', output)
        with wave.open(str(output)) as reader:
            require(reader.getnchannels() == 2 and reader.getframerate() == 44100,
                    'Rendered WAV is not 44.1 kHz stereo.')
            frames = reader.getnframes()
            require(frames > 0 and any(reader.readframes(frames)), 'Rendered PCM is empty or silent.')
        return {'project': str(path), 'wave': str(output), 'frames': frames,
                'sha256': sha256(output), 'clip_id': clip['id'],
                'length_ticks': clip['length_ticks'],
                'scope': 'First imported-audio clip only, at tick zero, at most one bar; '
                         'original gain, trim, track settings, and tempo retained.'}

    def verify_exact_limit(self):
        song = self.root / 'exact-500MB.mix'
        expected = write_exact_fixture(song)
        self.report['synthetic_fixture'] = {'path': str(song), 'bytes': song.stat().st_size,
                                            'sha256': sha256(song), 'waves': expected}
        project = self.convert_and_reopen(song, self.root / 'exact-500MB.mixjam')
        require(len(project['clips']) == 3, 'Exact-limit fixture did not preserve three clips.')
        require(len(project['imported_samples']) == 3,
                'Exact-limit fixture did not preserve three distinct imported WAVs.')
        observed = [{'bytes': Path(item['path']).stat().st_size,
                     'sha256': sha256(Path(item['path']))} for item in project['imported_samples']]
        require(sorted((item['bytes'], item['sha256']) for item in observed)
                == sorted((item['bytes'], item['sha256']) for item in expected),
                'Preserved WAV sizes or SHA-256 hashes differ from embedded bytes.')
        self.report['synthetic_fixture']['preserved_waves'] = observed
        self.report['synthetic_fixture']['json_equal_after_reopen'] = True
        return self.render_short(project, 'synthetic')

    def verify_over_limit(self):
        song = self.root / 'over-500MB.mix'
        with song.open('xb') as output:
            mark_sparse(output)
            output.write(struct.pack('<I', 0x2571))
            output.seek(LIMIT)
            output.write(b'\0')
        destination = self.root / 'over-500MB.mixjam'
        result = self.run('convert', '--mix', song, '--output', destination, success=False)
        require('500 MB' in result.stdout + result.stderr, 'Size-limit diagnostic is missing.')
        require(not destination.exists(), 'Rejected file created an output project.')
        return {'path': str(song), 'bytes': song.stat().st_size, 'output_absent': True}

    def verify_real_fixtures(self):
        songs = sorted((path for path in self.fixtures.rglob('*')
                        if path.is_file() and path.suffix.lower() == '.mix'),
                       key=lambda path: str(path).lower())
        require(songs, 'No MIX fixtures were found.')
        for index, song in enumerate(songs):
            record = {'path': str(song), 'bytes': song.stat().st_size, 'passed': False,
                      'clips': None, 'imported_wavs': None, 'imported_wave_bytes': None,
                      'command_start_index': len(self.report['commands'])}
            self.report['real_fixtures'].append(record)

            def verify_song():
                record['sha256'] = sha256(song)
                destination = self.root / f'real-{index:03d}.mixjam'
                project = self.convert_and_reopen(song, destination, self.samples)
                record.update(clips=len(project['clips']),
                              imported_wavs=len(project['imported_samples']),
                              saved_project=str(destination),
                              imported_wave_bytes=sum(Path(item['path']).stat().st_size
                                                      for item in project['imported_samples']))
                self.successful.append((record['bytes'], song, project))
                return {'saved_project': str(destination), 'json_equal_after_reopen': True}

            fixture_start = time.monotonic()
            record['passed'] = self.check(f'real_fixture:{song.relative_to(self.fixtures)}', verify_song,
                                          required=False)
            record['seconds'] = time.monotonic() - fixture_start
            record['command_end_index_exclusive'] = len(self.report['commands'])
            record['error'] = self.report['checks'][-1].get('error')
            self.save_report()
        embedded = [item for item in self.successful if item[2]['imported_samples']]
        if embedded:
            _, song, project = max(embedded, key=lambda item: item[0])
            self.report['real_render_source'] = str(song)
            self.check('largest_successful_real_import_short_render',
                       lambda: self.render_short(project, 'real', self.samples), required=False)
        else:
            self.check('largest_successful_real_import_short_render', lambda: require(
                False, 'No real fixture with imported audio passed conversion and reopen.'),
                required=False)

        self.report['real_fixtures_passed'] = not self.report['real_fixture_failures']

    def execute(self):
        try:
            require(self.exe_source.is_file(), 'Executable path is not a file.')
            require(self.args.timeout > 0, 'Command timeout must be positive.')
            if self.fixtures is not None:
                require(self.fixtures.is_dir(), 'Fixture folder does not exist.')
                require(self.samples.is_dir(), 'Sample folder does not exist.')
            shutil.copy2(self.exe_source, self.exe)
            self.report['exe_sha256'] = sha256(self.exe)
            self.check('empty_startup_library', lambda: require(
                json.loads(self.run('inspect').stdout)['samples'] == 0,
                'Copied executable discovered samples without a provider.'))
            self.check('exact_500MB_import_preservation_reopen_and_short_render', self.verify_exact_limit)
            self.check('500MB_plus_one_byte_rejected_without_output', self.verify_over_limit)
            if self.fixtures is not None:
                self.verify_real_fixtures()
        except Exception as error:
            self.report['failures'].append({'check': 'setup_or_execution',
                                            'error': f'{type(error).__name__}: {error}'})
        finally:
            self.report['passed'] = not self.report['failures'] and bool(self.report['checks'])
            self.save_report()
            if self.args.clean:
                shutil.rmtree(self.root, ignore_errors=True)
            print(json.dumps({'passed': self.report['passed'], 'report': str(self.report_path),
                              'artifacts': None if self.args.clean else str(self.root),
                              'real_fixtures_passed': self.report['real_fixtures_passed'],
                              'seconds': self.report['seconds'], 'failures': self.report['failures'],
                              'real_fixture_failures': self.report['real_fixture_failures']},
                             indent=2), flush=True)
        return 0 if self.report['passed'] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--fixtures', type=Path,
                        help='Optional folder containing real MIX files; searched recursively. '
                             'Requires --samples.')
    parser.add_argument('--samples', type=Path,
                        help='WAV-only sample folder used for real fixture conversion.')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path,
                        help='Folder for the copied executable, fixtures, and outputs. It must '
                             'not exist yet (default: a new folder in the system temp directory).')
    parser.add_argument('--clean', action='store_true',
                        help='Delete the artifact folder after the report is written.')
    parser.add_argument('--timeout', type=float, default=1200,
                        help='Maximum seconds per executable command (default: 1200).')
    args = parser.parse_args()
    if (args.fixtures is None) != (args.samples is None):
        parser.error('--fixtures and --samples must be given together.')
    try:
        return Verifier(args).execute()
    except ValueError as error:
        parser.error(str(error))


if __name__ == '__main__':
    raise SystemExit(main())

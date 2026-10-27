#requires -Version 7.0
param(
    [string]$Executable = '',
    [string]$OutputDirectory = '',
    [ValidateSet('1440x1000', '1024x768', '960x720')]
    [string[]]$Sizes = @('1440x1000', '1024x768', '960x720'),
    [ValidateSet('arrangement', 'sample-details', 'transport-focus', 'mixer', 'mixer-dense', 'mixer-empty', 'mixer-states', 'libraries', 'library-tempos', 'library-expanded', 'library-providers', 'follow', 'step-synth', 'step-synth-effects', 'effects', 'effects-expanded', 'master', 'master-active', 'master-reordered', 'master-end', 'master-metering', 'fx-board', 'clip-fades', 'automation', 'edit', 'stretch', 'help', 'unsaved', 'import', 'recorded-take', 'import-details', 'first-run', 'sparse', 'track64', 'missing', 'missing-details', 'missing-effects', 'missing-edit', 'missing-stretch', 'missing-step-synth', 'sample-conflicts')]
    [string[]]$Views = @('arrangement', 'sample-details', 'transport-focus', 'mixer', 'mixer-dense', 'mixer-empty', 'mixer-states', 'libraries', 'library-tempos', 'library-expanded', 'library-providers', 'follow', 'step-synth', 'step-synth-effects', 'effects', 'effects-expanded', 'master', 'master-active', 'master-reordered', 'master-end', 'master-metering', 'fx-board', 'clip-fades', 'automation', 'edit', 'stretch', 'help', 'unsaved', 'import', 'recorded-take', 'import-details', 'first-run', 'sparse', 'track64', 'missing', 'missing-details', 'missing-effects', 'missing-edit', 'missing-stretch', 'missing-step-synth', 'sample-conflicts')
)

$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $Executable) { $Executable = Join-Path $projectRoot 'target\release\mixjam-techno.exe' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $projectRoot 'tmp\evidence\ui\redesign' }
$Executable = [IO.Path]::GetFullPath($Executable)
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
    throw 'Build the native release executable before UI verification.'
}

# Python runs capture children with an isolated environment and a fixed timeout.
# Each run has a new directory. Existing evidence and user settings stay intact.
@'
from copy import deepcopy
from datetime import datetime, timezone
from hashlib import sha256
import json
import math
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import time
import wave

exe = Path(sys.argv[1]).resolve()
base = Path(sys.argv[2]).resolve()
sizes = sys.argv[3].split(',')
views = sys.argv[4].split(',')
root = base / datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
root.mkdir(parents=True, exist_ok=False)
sounds = root / 'samples'

definitions = [
    ('Drum', 'Analog kick', 55, 1),
    ('Drum', 'Broken rhythm', 110, 2),
    ('Drum', 'Closed hats', 880, 1),
    ('Bass', 'Low orbit', 65, 2),
    ('Bass', 'Rubber pulse', 82, 2),
    ('Sequence', 'Glass sequence', 220, 4),
    ('Sequence', 'Prism arpeggio', 330, 2),
    ('Sphere', 'Night atmosphere', 165, 4),
    ('Effect', 'Signal transition', 440, 1),
    ('Voice', 'Voce și ritm', 275, 2),
    ('Loop', 'Late night loop', 125, 2),
    ('Loop', 'Motor city', 180, 4),
]
sample_rate = 22050
samples = []
for index, (group, name, frequency, bars) in enumerate(definitions):
    folder = sounds / group
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / f'{name}.wav'
    frame_count = round(sample_rate * 240 / 140 * bars)
    audio = bytearray(frame_count * 2)
    for frame in range(frame_count):
        phase = frame / sample_rate
        envelope = math.exp(-12 * (phase % (60 / 140)))
        value = int(6000 * envelope * math.sin(2 * math.pi * frequency * phase))
        struct.pack_into('<h', audio, frame * 2, value)
    with wave.open(str(path), 'wb') as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(sample_rate)
        output.writeframes(audio)
    samples.append(dict(id=1000000 + index, name=name, group_name=group,
                        path=str(path), source_bpm=140, origin=None))

project = dict(schema_version=2, title='Night Shift', author='UI verification',
               bpm=140, master_gain=0.75, pitch_ratio=1,
               tracks=[dict(gain=0.8, pan=0, left_enabled=True, right_enabled=True)
                       for _ in range(64)],
               clips=[], loop_region=[0, 16 * 256], imported_samples=samples,
               import_notices=[], source_notices=[], libraries=[])
for track, sample_index in enumerate((0, 2, 1, 3, 5, 7, 9, 8)):
    bars = definitions[sample_index][3]
    for start in range(0 if track < 4 else 4, 16, bars):
        project['clips'].append(dict(id=len(project['clips']) + 1,
            sample_id=samples[sample_index]['id'], track=track,
            start_tick=start * 256, length_ticks=bars * 256,
            offset_frames=0, gain=1))

def write_project(name, value):
    path = root / f'{name}.json'
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')
    return path

arrangement = write_project('arrangement', project)
metadata_project = deepcopy(project)
metadata_project['imported_samples'][0]['name'] = 'Știință și ritm — atmosphère originală pentru o noapte foarte lungă'
metadata_details = write_project('sample-details', metadata_project)
import_details_project = deepcopy(project)
import_details_project['title'] = 'Imported song details'
import_details_project['import_notices'] = [dict(source_bpm=146, target_bpm=140)]
import_details_project['source_notices'] = [
    'MixJam v7: the master bus plays through the mastering pedals and the return buses through the FX buses; clip edge fades are not rendered.',
    'MixJam v7 source controls: source 1; saved gain trim 0.75; edge fade 12 ms.',
]
import_details = write_project('import-details', import_details_project)
missing_project = deepcopy(project)
missing_project.update(schema_version=3, title='Missing source recovery', loop_region=[0, 16 * 256])
missing_project['missing_samples'] = [dict(id=1001000, name='Original tape atmosphere', source_bpm=140,
    sample_rate=44100, nominal_ticks=4 * 256, length_inferred=False,
    reference=dict(type='external', path=r'Z:\Original sessions\Dance3\MIXWAVES\atmosphere.wav', origin='d3'),
    reason='The original WAV file is unavailable.',
    metadata={'Native label 1':'Original tape', 'Native label 2':'atmosphere',
              'Saved sample ID':'10204', 'Declared bars':'4',
              'Native metadata word 1':'12', 'Native metadata word 2':'14986'})]
for track, start in [(0, 0), (1, 4), (6, 8)]:
    missing_project['clips'].append(dict(id=1000+track, sample_id=1001000, track=track,
        start_tick=start * 256, length_ticks=4 * 256, offset_frames=0, gain=1))
missing = write_project('missing', missing_project)
conflicts_project = deepcopy(project)
conflicts_project.update(schema_version=3, title='Choose matching samples')
conflicts_project['imported_samples'][2]['path'] = samples[0]['path']
conflicts_project['sample_conflicts'] = [dict(
    reference=r'D:\Previous sample collection\Drum\Analog kick.wav',
    sample_ids=[samples[0]['id'], samples[2]['id']],
    candidates=[samples[0]['path'], samples[1]['path']], selected=0, confirmed=False,
    source_spans={})]
sample_conflicts = write_project('sample-conflicts', conflicts_project)
sparse_project = deepcopy(project)
sparse_project['title'] = 'Sparse track identity'
sparse_project['clips'] = [dict(id=i + 1, sample_id=samples[i]['id'], track=track,
    start_tick=0, length_ticks=512, offset_frames=0, gain=1)
    for i, track in enumerate((0, 63))]
sparse = write_project('sparse', sparse_project)
full_project = deepcopy(project)
full_project['title'] = 'All 64 tracks'
full_project['clips'] = [dict(id=track + 1, sample_id=samples[track % len(samples)]['id'],
    track=track, start_tick=(track % 4) * 256, length_ticks=512,
    offset_frames=0, gain=1) for track in range(64)]
full = write_project('full64', full_project)
empty_project = deepcopy(project)
empty_project['title'] = 'Empty mixer'
empty_project['clips'] = []
empty = write_project('empty', empty_project)
mixer_project = deepcopy(project)
mixer_project['title'] = 'Mixer states'
for index, gain in enumerate((-0.33333334, 0, 1, 4, -2, 0.25, 0.8, 1.5)):
    mixer_project['tracks'][index]['gain'] = gain
    mixer_project['tracks'][index]['pan'] = (-1, -0.5, 0, 0.5, 1, 0, 0, 0)[index]
mixer_project['tracks'][1].update(left_enabled=False, right_enabled=False)
mixer_project['tracks'][2]['right_enabled'] = False
mixer_project['tracks'][3]['left_enabled'] = False
# Track 2 shows No output, track 5 Muted, track 6 its pan, and the rest Solo elsewhere.
mixer_project['tracks'][4]['muted'] = True
mixer_project['tracks'][5]['solo'] = True
mixer_project['master_gain'] = 0.7777778
# A track can contain more than one sound; its strip must not invent one identity.
mixer_project['clips'][0]['sample_id'] = samples[4]['id']
# Track 1 and track 3 feed the FX buses; FX 1 holds a reverb and FX 2 a delay.
echoform = dict(type='echoform-delay', mode='sync', divisionL='1/4', divisionR='1/8.',
                timeMsL=420.0, timeMsR=610.0, feedback=68.0, pingPong=True, width=142.0,
                lowCut=160.0, highCut=7800.0, modRate=0.38, modDepth=5.4, character='tape',
                drive=0.0, duckAmount=34.0, duckRelease=620.0, outputDb=-1.5, bypass=False)
aetherform = dict(type='aetherform-reverb', spaceModel='chamber', preDelayMs=24.0,
                  decaySeconds=2.8, sizePercent=68.0, character='vintage', drivePercent=0.0,
                  widthPercent=148.0, lateBalancePercent=72.0, lowCutHz=180.0, highCutHz=8600.0,
                  diffusionPercent=78.0, densityPercent=84.0, earlyReflectionsEnabled=True,
                  modRateHz=0.32, modDepthPercent=18.0, shimmerEnabled=True,
                  shimmerAmountPercent=24.0, shimmerIntervalSemitones=12,
                  duckAmountPercent=28.0, duckReleaseMs=720.0, outputDb=-1.5, bypass=False)
mixer_project['fx'] = [dict(module=module, powered=True, returnLevel=level, limiter=True)
                       for module, level in ((aetherform, 0.88), (echoform, 0.82),
                                             (dict(type='empty'), 1.0), (dict(type='empty'), 1.0))]
mixer_project['fx'][1]['powered'] = False
mixer_project['tracks'][0]['sends'] = [0.4, 0.0, 0.25, 0.0]
mixer_project['tracks'][2]['sends'] = [0.0, 0.7, 0.0, 1.0]
mixer_states = write_project('mixer-states', mixer_project)
# Faded clips, a pitched clip, and automation lanes on the first tracks.
shaped_project = deepcopy(project)
shaped_project['title'] = 'Fades and automation'
for clip in shaped_project['clips']:
    if clip['track'] < 4:
        clip['fades'] = {'in': {'ticks': 64 * (clip['track'] + 1), 'curve': 'equal-power'},
                         'out': {'ticks': 128, 'curve': ('linear', 'exponential')[clip['track'] % 2]}}
shaped_project['clips'][0]['pitch_cents'] = -175
shaped_project['tracks'][0]['automation'] = {'volume': [
    {'tick': 0, 'value': -60.0}, {'tick': 4 * 256, 'value': 0.0},
    {'tick': 10 * 256, 'value': 0.0}, {'tick': 12 * 256, 'value': -18.0}]}
shaped_project['tracks'][1]['automation'] = {'volume': [
    {'tick': 2 * 256, 'value': 3.0}, {'tick': 6 * 256, 'value': -9.5}],
    'filter': [{'tick': 0, 'value': -0.8}, {'tick': 8 * 256, 'value': 0.0}]}
shaped_project['tracks'][2]['sends'] = [0.3, 0.0, 0.0, 0.0]
shaped_project['tracks'][2]['automation'] = {'sends': [[], [{'tick': 0, 'value': 0.0}, {'tick': 8 * 256, 'value': 1.0}], [], []]}
shaped = write_project('shaped', shaped_project)
follow_project = deepcopy(project)
follow_project['title'] = 'Follow the playhead'
follow_project['loop_region'] = [32 * 256, 48 * 256]
for clip in follow_project['clips']:
    clip['start_tick'] += 32 * 256
follow = write_project('follow', follow_project)
tempo_sounds = root / 'folder-tempos'
if 'library-tempos' in views:
    for index in range(60):
        folder = tempo_sounds / f'Folder {index:02}'
        folder.mkdir(parents=True)
        for sample in samples[:3]:
            shutil.copyfile(sample['path'], folder / Path(sample['path']).name)
large_sounds = root / 'large-library'
if 'library-expanded' in views:
    for index in range(128):
        source = samples[index % len(samples)]
        destination = large_sounds / source['group_name'] / f'Sound {index + 1:03}.wav'
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source['path'], destination)

report = dict(executable=str(exe), executable_sha256=sha256(exe.read_bytes()).hexdigest(),
              captured_utc=datetime.now(timezone.utc).isoformat(), platform=platform.platform(),
              fixture='12 synthetic WAVs at 22050 Hz, schema-2 native projects, 8/2/64 used tracks; schema-3 missing sources and sample conflict choices; import notices; expanded library adds 128 named copies; library-tempos uses an empty project and 60 folders with 3 WAVs each',
              output_directory=str(root), captures=[],
              checks=['Native process exits successfully', 'BMP exists and dimensions match',
                      'Dedicated per-case user data directory'],
              gaps=['Visual content requires image review', 'No microphone or physical listening check',
                    'Recorded-take capture checks presentation only; it does not simulate microphone input',
                    'No native file-picker or screen-reader interaction',
                    'No high-DPI or clean Windows VM check'])

for size in sizes:
    for view in views:
        name = f'{view}-{size}'
        capture = root / f'{name}.bmp'
        env = os.environ.copy()
        for key in ('MIXJAM_TECHNO_CAPTURE_TO', 'MIXJAM_TECHNO_CAPTURE_VIEW', 'MIXJAM_TECHNO_WINDOW_SIZE',
                    'MIXJAM_TECHNO_REFERENCE_SAMPLES'):
            env.pop(key, None)
        env.update(MIXJAM_TECHNO_CAPTURE_TO=str(capture), MIXJAM_TECHNO_WINDOW_SIZE=size,
                   MIXJAM_TECHNO_CAPTURE_VIEW='' if view in ('arrangement', 'sparse', 'first-run') else view,
                   MIXJAM_TECHNO_DATA_DIR=str(root / 'user-data' / name))
        command = [str(exe), 'gui']
        if view != 'first-run':
            fixture = {
                'sparse': sparse, 'track64': full, 'mixer-dense': full,
                'mixer-empty': empty, 'mixer-states': mixer_states, 'follow': follow,
                'fx-board': mixer_states, 'clip-fades': shaped, 'automation': shaped,
                'missing': missing, 'missing-details': missing,
                'missing-effects': missing, 'missing-edit': missing,
                'missing-stretch': missing, 'missing-step-synth': missing,
                'import-details': import_details, 'sample-details': metadata_details,
                'sample-conflicts': sample_conflicts,
            }.get(view, arrangement)
            folder = tempo_sounds if view == 'library-tempos' else large_sounds if view == 'library-expanded' else sounds
            command += ['--samples', str(folder)]
            if view != 'library-tempos':
                command += ['--mix', str(fixture)]
        started = time.monotonic()
        entry = dict(name=name, view=view, requested_size=size, path=str(capture), command=command)
        try:
            result = subprocess.run(command, cwd=root, env=env, capture_output=True, timeout=45,
                                    creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            (root / f'{name}.stdout.txt').write_bytes(result.stdout)
            (root / f'{name}.stderr.txt').write_bytes(result.stderr)
            entry['exit_code'] = result.returncode
            assert result.returncode == 0, f'Native process returned {result.returncode}'
            assert capture.is_file(), 'The native process produced no capture'
            data = capture.read_bytes()
            assert data[:2] == b'BM', 'Capture is not a BMP'
            width, height = struct.unpack_from('<ii', data, 18)
            entry['dimensions'] = [width, abs(height)]
            entry['sha256'] = sha256(data).hexdigest()
            assert entry['dimensions'] == list(map(int, size.split('x'))), 'Capture dimensions differ'
            entry['passed'] = True
        except (AssertionError, subprocess.TimeoutExpired) as error:
            entry.update(passed=False, error=str(error))
        entry['seconds'] = round(time.monotonic() - started, 3)
        report['captures'].append(entry)
        (root / 'captures.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
        print(f"{'PASS' if entry['passed'] else 'FAIL'} {name}: {entry['seconds']} s", flush=True)

report['passed'] = all(capture['passed'] for capture in report['captures'])
(root / 'captures.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(f"Report: {root / 'captures.json'}")
raise SystemExit(0 if report['passed'] else 1)
'@ | & python - $Executable $OutputDirectory ($Sizes -join ',') ($Views -join ',')
if ($LASTEXITCODE -ne 0) { throw 'Native UI capture verification failed. Inspect captures.json.' }

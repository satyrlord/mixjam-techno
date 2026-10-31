"""Write format-18 .mixjam songs from Python.

Bars and beats count from 1, as in the app. One bar is 256 ticks, one beat 64.
A sound is a WAV path; the writer links it as an imported sample by path.

Placement rules that the writer enforces, so two classic mistakes cannot happen:
- `loop` tiles a sound at its own length. A 1-bar loop over 8 bars is eight
  1-bar clips, never 2-bar clips that leave every second bar silent.
- `loop` and `phrase` end a melodic run with a fade out when the sound is
  still loud at its end, so it dies away instead of stopping dead.

Automation rules: a lane rests at its neutral value until its first point, and
saving refuses a lane that ends away from neutral while later clips still play
(the engine holds a lane's last value to the end of the song).

`Song.save` also writes `.meta.json` beside the song: lane names and roles, the
sections and the key. render.py, measure.py, audible.py and master.py read it.
"""
import json
import math
import wave
from datetime import datetime, timezone
from pathlib import Path

import numpy as np

TICKS_PER_BAR = 256
TICKS_PER_BEAT = 64
FORMAT_VERSION = 18
IMPORTED_ID_START = 1_000_000
TRACKS = 64

# Roles whose runs end with a fade when the sound is still loud at its end.
MELODIC = {'pad', 'lead', 'keys', 'arp', 'fx', 'vocal', 'atmos'}


def tick(bar, beat=1.0):
    """The tick of a 1-based bar and beat; fractions are allowed."""
    return int(round((bar - 1) * TICKS_PER_BAR + (beat - 1) * TICKS_PER_BEAT))


def db(amplitude):
    return 20 * math.log10(amplitude) if amplitude > 0 else -math.inf


def gain(decibels):
    return 10 ** (decibels / 20)


def write_wav(path, rate, data):
    """Write float samples (frames, channels) as a 16-bit PCM WAV, held to full scale."""
    ints = np.clip(np.round(np.asarray(data) * 32768), -32768, 32767).astype('<i2')
    with wave.open(str(path), 'wb') as w:
        w.setnchannels(ints.shape[1])
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(ints.tobytes())


def read_wav(path):
    """Rate and float samples (frames, channels) of a 16/24/32-bit PCM WAV."""
    with wave.open(str(path), 'rb') as w:
        rate, channels, width, frames = w.getframerate(), w.getnchannels(), w.getsampwidth(), w.getnframes()
        raw = w.readframes(frames)
    if width == 2:
        data = np.frombuffer(raw, '<i2').astype(np.float32) / 32768
    elif width == 3:
        b = np.frombuffer(raw, np.uint8).reshape(-1, 3).astype(np.int32)
        ints = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
        data = (np.where(ints >= 1 << 23, ints - (1 << 24), ints)).astype(np.float32) / (1 << 23)
    elif width == 4:
        data = np.frombuffer(raw, '<i4').astype(np.float32) / (1 << 31)
    else:
        raise ValueError(f'{path}: unsupported sample width {width}')
    return rate, data.reshape(-1, channels)


def ticks_of(frames, rate, source_bpm):
    """The song ticks that `frames` of a sound at `source_bpm` fill (the engine varispeeds it)."""
    return frames / rate * source_bpm / 240 * TICKS_PER_BAR


class Sound:
    """A WAV with its length in ticks and whether it ends loud."""

    def __init__(self, path, source_bpm):
        self.path = Path(path)
        rate, data = read_wav(self.path)
        self.rate = rate
        self.frames = len(data)
        self.source_bpm = source_bpm
        self.ticks = ticks_of(self.frames, rate, source_bpm)
        mono = data.mean(axis=1)
        whole = float(np.sqrt(np.mean(mono ** 2)) + 1e-12)
        tail = mono[-int(rate * 0.05):]
        self.tail_db = db(float(np.sqrt(np.mean(tail ** 2)) + 1e-12)) - db(whole)
        self.peak_db = db(float(np.abs(data).max()) + 1e-12)

    @property
    def bars(self):
        return self.ticks / TICKS_PER_BAR

    @property
    def loop_bars(self):
        """Whole bars, or None when the sound is not close to a whole number of bars."""
        return loop_bars(self.bars)

    @property
    def ends_loud(self):
        """The last 50 ms are within 20 dB of the whole sound: it stops, not decays."""
        return self.tail_db > -20


def loop_bars(bars):
    """Whole bars of a sound `bars` long, or None when it is more than 3% away from a whole number."""
    whole = max(1, round(bars))
    return whole if abs(bars - whole) <= 0.03 * whole else None


class Song:
    def __init__(self, title, bpm=140.0, author='', key=None):
        """`key` names the song's scale for the reports, for example 'A minor' or 'C major'."""
        self.title, self.bpm, self.author, self.key = title, float(bpm), author, key
        self.lanes = [dict(gain=1.0, polarityInverted=False, pan=0.0, leftEnabled=True, rightEnabled=True,
                           muted=False, solo=False, sends=[0.0] * 4, placements=[]) for _ in range(TRACKS)]
        self.automation = [dict() for _ in range(TRACKS)]
        self.samples = {}
        self.sounds = {}
        self.tempos = {}
        self.roles = {}
        self.names = {}
        self.next_clip = 1
        self.fx = [dict(module={'type': 'empty'}, powered=True, returnLevel=1.0, limiter=True) for _ in range(4)]
        self.master = board()
        self.master_gain = 1.0
        self.sections = []
        self.palette = self.palette_path = self.palette_roots = None

    def use_palette(self, path):
        """Hold the song to a palette (`palette.py make`): a sound outside its families stops the script."""
        import palette as palettes
        chosen = palettes.load_palette(path)
        roots = palettes.roots()
        for sound in self.sounds:
            if not palettes.allowed(chosen, sound, roots):
                raise ValueError(f'{sound} is outside the palette {path}')
        self.palette_path = Path(path)
        self.palette = chosen
        self.palette_roots = roots

    # Sounds and lanes.
    def tempo(self, path, bpm):
        """Declare a sound's own tempo before it is placed; the engine varispeeds it to the song's."""
        self.tempos[str(Path(path))] = float(bpm)
        return path

    def sound(self, path, source_bpm=None):
        """The Sound for `path`, linked once. Its tempo: tempo(), else `source_bpm`, else the song's."""
        path = str(Path(path))
        source_bpm = self.tempos.get(path, source_bpm or self.bpm)
        if self.palette is not None:
            import palette as palettes  # tools/song/palette.py; only songs that keep a palette need it
            if not palettes.allowed(self.palette, path, self.palette_roots):
                raise ValueError(f'{path} is outside the palette {self.palette_path}')
        if path not in self.sounds:
            self.sounds[path] = Sound(path, source_bpm)
            sample_id = IMPORTED_ID_START + len(self.samples)
            self.samples[path] = dict(id=sample_id, name=Path(path).stem, groupName=self.title,
                                      path=path, sourceBpm=source_bpm,
                                      sourceSampleRate=self.sounds[path].rate, origin=None)
        return self.sounds[path]

    def lane(self, track, name, role, gain_db=0.0, pan=0.0, sends=(0, 0, 0, 0)):
        """Set up a 1-based track for one role. Gains are dB on the fader.

        A lane named "NAME (twin)" continues lane "NAME" (cross-faded chord changes); the
        reports count the two as one lane.
        """
        lane = self.lanes[track - 1]
        lane.update(gain=gain(gain_db), pan=pan, sends=[float(s) for s in sends])
        self.roles[track] = role
        self.names[track] = name

    def eq(self, track, **settings):
        """Switch on a lane's channel EQ. Settings use the song's keys: lowCut, lowHz, lowDb,
        midHz, midDb, midQ, the second peak's peak2Hz, peak2Db, peak2Q, highHz, highDb,
        highCut; on=False disables it. The rest keep a new track's values; the second peak's
        keys are written only when they differ from a new track's, as the app saves them."""
        eq = dict(EQ_DEFAULT, **PEAK2_DEFAULT, on=True)
        unknown = set(settings) - set(eq)
        if unknown:
            raise ValueError(f'unknown EQ settings {sorted(unknown)}')
        eq.update({key: bool(value) if key == 'on' else float(value) for key, value in settings.items()})
        for key, (low, high) in EQ_RANGES.items():
            if not low <= eq[key] <= high:
                raise ValueError(f'track {track}: EQ {key} {eq[key]:g} is outside {low:g} to {high:g}')
        for key, default in PEAK2_DEFAULT.items():
            if eq[key] == default:
                del eq[key]
        self.lanes[track - 1]['eq'] = eq

    def compressor(self, track, threshold=-20.0, ratio=4.0, attack=10.0, release=100.0, makeup=0.0,
                   on=True):
        """Switch on a lane's compressor, after its EQ and before its width, automation and
        fader: above `threshold` dBFS (the mean power of both channels) the level comes down
        by `ratio`, with a 6 dB soft knee; attack and release in ms, `makeup` in dB after it."""
        _whole_track(track)
        settings = dict(threshold=threshold, ratio=ratio, attack=attack, release=release, makeup=makeup)
        settings = {name: float(value) for name, value in settings.items()}
        for name, (low, high) in COMPRESSOR_RANGES.items():
            if not low <= settings[name] <= high:
                raise ValueError(f'track {track}: compressor {name} is outside {low:g} to {high:g}')
        self.lanes[int(track) - 1]['compressor'] = dict(on=bool(on), **settings)

    def width(self, track, percent):
        """Set a lane's stereo width before its pan: 0 is mono, 100 leaves the lane as it is,
        200 doubles its side. It needs both outputs on. 100 removes the setting."""
        _whole_track(track)
        percent = float(percent)
        low, high = WIDTH_RANGE
        if not low <= percent <= high:
            raise ValueError(f'track {track}: width {percent:g} is outside {low:g} to {high:g}%')
        lane = self.lanes[int(track) - 1]
        if percent == 100.0:
            lane.pop('width', None)
        else:
            lane['width'] = percent

    def ducker(self, track, key, threshold=-30.0, depth=6.0, attack=2.0, release=120.0):
        """Switch on a lane's ducker: after its level, the lane drops by as much as lane `key`'s
        peak rises over `threshold` dBFS, by at most `depth` dB; attack and release in ms. The key
        is the key lane as it sounds (after its fader), before its own ducker and the master gain."""
        track, key = _whole_track(track), _whole_track(key, 'key')
        if key == track:
            raise ValueError(f'track {track}: a ducker needs another track as its key')
        settings = dict(threshold=float(threshold), depth=float(depth), attack=float(attack),
                        release=float(release))
        for name, (low, high) in DUCKER_RANGES.items():
            if not low <= settings[name] <= high:
                raise ValueError(f'track {track}: ducker {name} is outside {low:g} to {high:g}')
        self.lanes[track - 1]['ducker'] = dict(on=True, key=key, **settings)

    # Placement.
    def clip(self, track, path, start_tick, length_ticks, offset_frames=0, gain_db=0.0,
             fade_in=0, fade_out=0, curve_in='linear', curve_out='exponential', pitch_cents=0):
        """One clip in ticks. Fades are in ticks; they are shortened to fit the clip."""
        self.sound(path)
        fade_in = min(int(fade_in), length_ticks)
        fade_out = min(int(fade_out), length_ticks - fade_in)
        placement = dict(id=self.next_clip, sampleId=self.samples[str(Path(path))]['id'],
                         startTick=int(start_tick), durationTicks=int(length_ticks),
                         offsetFrames=int(offset_frames), gain=gain(gain_db))
        if fade_in or fade_out:
            placement['fades'] = {'in': {'ticks': fade_in, 'curve': curve_in},
                                  'out': {'ticks': fade_out, 'curve': curve_out}}
        if pitch_cents:
            placement['pitchCents'] = int(pitch_cents)
        self.lanes[track - 1]['placements'].append(placement)
        self.next_clip += 1
        return placement

    def loop(self, track, path, first_bar, last_bar, gain_db=0.0, fade_in_beats=0.0,
             fade_out_beats=None, pitch_cents=0, size=None, ring=False, join_fade=0):
        """Tile a loop over bars first_bar..last_bar (inclusive) at its own length.

        `size` tiles a loop that carries a release tail (an 8-bar loop saved as
        9-10 bars) every `size` bars. `ring` lets the last tile play its tail
        past last_bar instead of being cut there. `join_fade` fades each join
        between tiles out and in over that many ticks, for a loop whose end does
        not meet its start.
        """
        sound = self.sound(path)
        size = size or sound.loop_bars
        if size is None:
            raise ValueError(f'{Path(path).name} is {sound.bars:.3f} bars, not a loop; use phrase() or size=')
        if size > sound.bars + 0.03:
            raise ValueError(f'{Path(path).name} is {sound.bars:.3f} bars, shorter than size {size}')
        start, end = tick(first_bar), tick(last_bar + 1)
        placements = []
        at = start
        while at < end:
            length = min(size * TICKS_PER_BAR, end - at)
            placements.append(self.clip(track, path, at, length, gain_db=gain_db, pitch_cents=pitch_cents))
            at += length
        last = placements[-1]
        cut = last['durationTicks'] < sound.ticks - 4
        if ring and cut:
            last['durationTicks'] = int(sound.ticks)
            cut = False
        for before, after in zip(placements, placements[1:]):
            set_fade(before, 'out', join_fade, 'equal-power')
            set_fade(after, 'in', join_fade, 'equal-power')
        self._edges(track, sound, placements, fade_in_beats, fade_out_beats, cut=cut)
        return placements

    def phrase(self, track, path, bar, beat=1.0, bars=None, gain_db=0.0, fade_in_beats=0.0,
               fade_out_beats=None, pitch_cents=0, offset_frames=0):
        """Place a sound once, at its own length or cut to `bars`."""
        sound = self.sound(path)
        full = sound.ticks - ticks_of(offset_frames, sound.rate, sound.source_bpm)
        length = int(round(full)) if bars is None else int(round(bars * TICKS_PER_BAR))
        placement = self.clip(track, path, tick(bar, beat), length, offset_frames=offset_frames,
                              gain_db=gain_db, pitch_cents=pitch_cents)
        self._edges(track, sound, [placement], fade_in_beats, fade_out_beats, cut=length < full - 1)
        return placement

    def _edges(self, track, sound, placements, fade_in_beats, fade_out_beats, cut):
        first, last = placements[0], placements[-1]
        role = self.roles.get(track, '')
        if fade_out_beats is None:
            # A melodic run that stops loud, or is cut short, cuts cleanly: one tick, against a click.
            # A fade is a clean cut or a long fade; ask for a long one with fade_out_beats.
            fade_out_beats = 1 / TICKS_PER_BEAT if role in MELODIC and (sound.ends_loud or cut) else 0.0
        set_fade(first, 'in', fade_in_beats * TICKS_PER_BEAT, 'linear')
        set_fade(last, 'out', fade_out_beats * TICKS_PER_BEAT, 'exponential')

    # Automation, effects, master.
    def automate(self, track, lane, points):
        """lane: volume, send1..send4, or filter. points: [(bar, value)], bar 1-based, fractions allowed.

        Volume is dB over the fader; a send replaces the knob (0-1); filter is -1 (low-pass) to 1
        (high-pass), 0 open. Points add to the lane's earlier points, so each section writes its
        own moves; a point on a tick that the lane already has is an error.
        """
        automation = self.automation[track - 1]
        if lane.startswith('send'):
            entries = automation.setdefault('sends', [[], [], [], []])[int(lane[4:]) - 1]
        else:
            entries = automation.setdefault(lane, [])
        ticks = {entry['tick'] for entry in entries}
        for bar, value in points:
            if tick(bar) in ticks:
                raise ValueError(f'track {track} {lane}: a second point at bar {bar}')
            ticks.add(tick(bar))
            entries.append(dict(tick=tick(bar), value=float(value)))
        entries.sort(key=lambda entry: entry['tick'])

    def bus(self, index, module, return_level=None, powered=True, limiter=True):
        """index 1-4; module from delay()/reverb(); return_level 0-1 (default: the preset's).
        A module's `duckKey` (a track from 1 to 64) ducks the effect while that track sounds,
        instead of its own input; 0 is its own input and is not written."""
        module = dict(module)
        if 'duckKey' in module:
            module['duckKey'] = _key(module['duckKey'], f'FX {index} duckKey')
            if not module['duckKey']:
                del module['duckKey']
        level = module.pop('_return', 1.0) if return_level is None else return_level
        module.pop('_return', None)
        self.fx[index - 1] = dict(module=module, powered=powered, returnLevel=level, limiter=limiter)

    def _held(self, index):
        """Track `index`'s automation as the engine should play it (see the module rules)."""
        lane = self.lanes[index]
        held = {}
        for key, points in self.automation[index].items():
            lists = points if key == 'sends' else [points]
            out = []
            for i, entries in enumerate(lists):
                neutral = lane['sends'][i] if key == 'sends' else 0.0
                name = key if key != 'sends' else f'send{i + 1}'
                out.append(hold(entries, neutral, lane['placements'], f'track {index + 1} {name}'))
            held[key] = out if key == 'sends' else out[0]
        return held

    def key_tracks(self, master=True):
        """The tracks that active duckers, active master pedals, and FX effects listen to.
        A bypassed board, or master=False for the premaster, uses no master keys.
        The app allows at most MAX_KEYS together."""
        keys = {lane['ducker']['key'] for lane in self.lanes if lane.get('ducker', {}).get('on')}
        if master and not self.master.get('bypass', False):
            keys |= {settings['key'] for name, settings in self.master.items()
                     if name in KEYED_PEDALS and settings.get('on') and settings.get('key')}
        keys |= {bus['module']['duckKey'] for bus in self.fx if bus['module'].get('duckKey')}
        return keys

    # Output.
    def document(self, master=True):
        keys = self.key_tracks(master=master)
        if len(keys) > MAX_KEYS:
            raise ValueError(f'the duckers, pedals and FX use {len(keys)} key tracks; '
                             f'the app allows {MAX_KEYS}')
        lanes = []
        for index, lane in enumerate(self.lanes):
            lane = dict(lane)
            if self.automation[index]:
                lane['automation'] = self._held(index)
            lane['placements'] = sorted(lane['placements'], key=lambda p: p['startTick'])
            lanes.append(lane)
        now = datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
        return {
            'formatVersion': FORMAT_VERSION, 'appVersion': 'tools-song',
            'createdAt': now, 'modifiedAt': now,
            'song': dict(title=self.title, author=self.author, bpm=self.bpm,
                         masterGain=self.master_gain, loopRegion=None),
            'master': self.master if master else board_off(self.master), 'fxBuses': self.fx, 'lanes': lanes,
            'samples': list(self.samples.values()), 'missingSamples': [], 'sampleConflicts': [],
            'importNotices': [], 'sourceNotices': [], 'libraries': [],
        }

    def section(self, name, first_bar, bars):
        """Name a span of the form, for the reports."""
        self.sections.append(dict(name=name, first_bar=first_bar, bars=bars))

    def save(self, path, master=True):
        """Write the song and, beside it, `.meta.json` with lane names and roles, sections, tempo and key."""
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.document(master), indent=1), encoding='utf-8')
        meta = dict(lanes={str(t): dict(name=self.names[t], role=r) for t, r in self.roles.items()},
                    sections=self.sections, bpm=self.bpm, key=self.key)
        path.with_suffix('.meta.json').write_text(json.dumps(meta, indent=1), encoding='utf-8')
        return path


def _whole_track(value, name='track'):
    """A track number from 1 to 64, as an int."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not 1 <= value <= TRACKS \
            or value != int(value):
        raise ValueError(f'{name} must be a whole track number from 1 to {TRACKS}')
    return int(value)


def _key(value, name):
    """A pedal's or an effect's key: 0 for its own input, else a track from 1 to 64."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not 0 <= value <= TRACKS \
            or value != int(value):
        raise ValueError(f'{name} must be 0 (its own input) or a whole track number from 1 to {TRACKS}')
    return int(value)


def set_fade(placement, edge, ticks, curve):
    """Set one edge's fade of a placement, shortened so the two fades fit the clip."""
    ticks = int(round(ticks))
    if ticks <= 0:
        return
    fades = placement.setdefault('fades', {'in': {'ticks': 0, 'curve': 'linear'},
                                           'out': {'ticks': 0, 'curve': 'exponential'}})
    other = fades['out' if edge == 'in' else 'in']['ticks']
    fades[edge] = {'ticks': min(ticks, placement['durationTicks'] - other), 'curve': curve}


def hold(entries, neutral, placements, label):
    """One automation lane as the engine should play it.

    The engine holds a lane's first value back to bar 1 and its last value to the end. The lane
    rests at `neutral` until its first point, and a lane that ends away from neutral while a
    later clip of the track still plays is an error.
    """
    entries = [dict(e) for e in entries]
    if entries and entries[0]['tick'] > 0 and entries[0]['value'] != neutral:
        rest = sorted({0, entries[0]['tick'] - 1})
        entries[:0] = [dict(tick=t, value=neutral) for t in rest]
    if entries and entries[-1]['value'] != neutral:
        later = [p for p in placements if p['startTick'] >= entries[-1]['tick']]
        if later:
            bar = min(p['startTick'] for p in later) / TICKS_PER_BAR + 1
            raise ValueError(f'{label}: holds {entries[-1]["value"]} over a clip at bar {bar:g}; '
                             f'end the lane at its neutral value')
    return entries


# A new track's channel EQ: the cuts at their ends are off, the bands flat.
EQ_DEFAULT = dict(on=False, lowCut=20.0, lowHz=100.0, lowDb=0.0, midHz=1000.0, midDb=0.0, midQ=1.0,
                  highHz=8000.0, highDb=0.0, highCut=20000.0)
# The second peak of a new track's EQ: flat. The song saves its keys only when they differ.
PEAK2_DEFAULT = dict(peak2Hz=250.0, peak2Db=0.0, peak2Q=1.0)
# The application's range of each EQ setting (`src/eq.rs`).
EQ_RANGES = dict(lowCut=(20.0, 2000.0), lowHz=(40.0, 500.0), lowDb=(-18.0, 18.0), midHz=(150.0, 10000.0),
                 midDb=(-18.0, 18.0), midQ=(0.3, 8.0), peak2Hz=(40.0, 16000.0), peak2Db=(-18.0, 18.0),
                 peak2Q=(0.3, 8.0), highHz=(1500.0, 16000.0), highDb=(-18.0, 18.0),
                 highCut=(1000.0, 20000.0))
DUCKER_RANGES = dict(threshold=(-60.0, 0.0), depth=(0.0, 24.0), attack=(0.1, 50.0), release=(10.0, 1000.0))
# The lane compressor's ranges (`src/compressor.rs`) and the lane width's (`TRACK_WIDTH`).
COMPRESSOR_RANGES = dict(threshold=(-60.0, 0.0), ratio=(1.0, 20.0), attack=(0.1, 100.0),
                         release=(10.0, 1000.0), makeup=(0.0, 24.0))
WIDTH_RANGE = (0.0, 200.0)
# The most tracks that the duckers, pedals and FX may listen to together (`MAX_KEYS`).
MAX_KEYS = 8

# The song-file keys of the three booster pedals. Each key is written here only, so that a
# rename in the song format is one edit.
# Format 15 and earlier songs keep their earlier keys; the application renames them on open.
BOOSTER_EQ = 'boosterEq'
BOOSTER_COMPRESSOR = 'boosterCompressor'
BOOSTER_WIDE = 'boosterWide'

# The pedals that may listen to a track instead of their own input (`key`).
KEYED_PEDALS = {BOOSTER_COMPRESSOR, 'busCompressor', 'multiband', 'limiter'}

# The master board of a new song: every pedal off at its default settings, the limiter last.
BOARD_ORDER = [BOOSTER_EQ, BOOSTER_COMPRESSOR, BOOSTER_WIDE, 'softClip', 'tube', 'trimEq', 'busCompressor',
               'maximizer', 'liftEq', 'tape', 'imaging', 'multiband', 'limiter']
PEDAL_DEFAULTS = {
    BOOSTER_EQ: dict(bands=[50] * 10), BOOSTER_COMPRESSOR: dict(drive=500, speed=500, gain=500),
    BOOSTER_WIDE: dict(spread=500), 'softClip': dict(amount=1.5, ceiling=-0.5), 'tube': dict(drive=2.5, mix=100.0),
    'trimEq': dict(highPass=20.0, mud=-1.5, harsh=-1.0),
    'busCompressor': dict(threshold=-16.0, ratio=2.0, attack=10.0, release=300.0),
    'maximizer': dict(boost=10.0), 'liftEq': dict(low=1.0, air=1.0), 'tape': dict(drive=2.0, speed=30),
    'imaging': dict(width=105.0, monoBelow=120.0), 'multiband': dict(low=20.0, mid=15.0, high=20.0),
    'limiter': dict(gain=4.0, ceiling=-1.0),
}


def board(trim=0.0, **pedals):
    """A master board. Each keyword names a pedal and gives the settings to change; naming a
    pedal switches it on unless the settings say on=False. Example:
    board(trim=-3, imaging=dict(width=100, monoBelow=120), limiter=dict(gain=4.5)).
    A dynamics pedal (KEYED_PEDALS) may take `key`, a track from 1 to 64 that it listens to
    instead of its own input; see `pedal_key`."""
    unknown = set(pedals) - set(PEDAL_DEFAULTS)
    if unknown:
        raise ValueError(f'unknown pedals {sorted(unknown)}')
    out = {'trim': float(trim), 'order': list(BOARD_ORDER)}
    for name, defaults in PEDAL_DEFAULTS.items():
        out[name] = dict(on=False, **json.loads(json.dumps(defaults)))
        if name in pedals:
            settings = dict(pedals[name])
            key = settings.pop('key', None)
            out[name].update({'on': True, **settings})
            if key is not None:
                pedal_key(out, name, key)
    return out


def pedal_key(master, pedal, key):
    """Let a dynamics pedal of a board listen to track `key` (1 to 64) as it sounds after its
    fader, instead of its own input; 0 returns it to its own input. Only the app's render of
    the song hears the key: master.py masters the premaster file on track 1 (see its help)."""
    if pedal not in KEYED_PEDALS:
        raise ValueError(f'{pedal} has no key; the keyed pedals are {sorted(KEYED_PEDALS)}')
    key = _key(key, f'{pedal} key')
    if key:
        master[pedal]['key'] = key
    else:
        master[pedal].pop('key', None)
    return master


def board_off(master):
    """A copy of a board with the trim at 0 and every pedal off: the premaster."""
    master = json.loads(json.dumps(master))
    master['trim'] = 0.0
    for name in master['order']:
        master[name]['on'] = False
    return master


def load_board(path, trim=None):
    """A board from a JSON file (the `master` object of a .mixjam); `trim` replaces its trim."""
    master = json.loads(Path(path).read_text(encoding='utf-8'))
    if trim is not None:
        master['trim'] = float(trim)
    return master


_DELAY = dict(type='echoform-delay', mode='sync', divisionL='1/4', divisionR='1/8.', timeMsL=420.0,
              timeMsR=610.0, feedback=68.0, pingPong=True, width=142.0, lowCut=160.0, highCut=7800.0,
              modRate=0.38, modDepth=5.4, character='tape', drive=0.0, duckAmount=34.0, duckRelease=620.0,
              outputDb=-1.5, bypass=False)
_DELAYS = {
    'Wide Tape Echo': ({}, 0.82),
    'Dotted Motion': (dict(divisionL='1/8.', divisionR='1/4T', timeMsL=430.0, timeMsR=350.0, feedback=54.0,
                           width=168.0, lowCut=240.0, highCut=9600.0, modRate=0.74, modDepth=2.8,
                           character='analog', duckAmount=46.0, duckRelease=480.0, outputDb=-1.8), 0.78),
    'Dub Feedback': (dict(divisionL='1/4.', divisionR='1/2T', timeMsL=680.0, timeMsR=870.0, feedback=90.0,
                          width=136.0, lowCut=180.0, highCut=6000.0, modRate=0.24, modDepth=7.6,
                          character='tape', drive=32.0, duckAmount=4.0, duckRelease=980.0, outputDb=0.0), 1.0),
}
_REVERB = dict(type='aetherform-reverb', spaceModel='chamber', preDelayMs=24.0, decaySeconds=2.8,
               sizePercent=68.0, character='vintage', drivePercent=0.0, widthPercent=148.0,
               lateBalancePercent=72.0, lowCutHz=180.0, highCutHz=8600.0, diffusionPercent=78.0,
               densityPercent=84.0, earlyReflectionsEnabled=True, modRateHz=0.32, modDepthPercent=18.0,
               shimmerEnabled=False, shimmerAmountPercent=24.0, shimmerIntervalSemitones=12,
               duckAmountPercent=28.0, duckReleaseMs=720.0, outputDb=-1.5, bypass=False)
_REVERBS = {
    'Warm Chamber': ({}, 0.88),
    'Dark Hall': (dict(spaceModel='hall', character='vintage', preDelayMs=38.0, decaySeconds=5.6, sizePercent=88.0,
                       widthPercent=164.0, lateBalancePercent=82.0, lowCutHz=260.0, highCutHz=4800.0,
                       diffusionPercent=70.0, densityPercent=76.0, modRateHz=0.21, modDepthPercent=24.0,
                       shimmerAmountPercent=18.0, shimmerIntervalSemitones=7, duckAmountPercent=18.0,
                       duckReleaseMs=1100.0, outputDb=-3.5), 0.92),
    'Ambient Bloom': (dict(spaceModel='hall', character='bloom', preDelayMs=72.0, decaySeconds=9.5,
                           sizePercent=96.0, widthPercent=188.0, lateBalancePercent=91.0, lowCutHz=340.0,
                           highCutHz=7200.0, diffusionPercent=94.0, densityPercent=98.0, modRateHz=0.14,
                           modDepthPercent=42.0, shimmerEnabled=True, shimmerAmountPercent=38.0,
                           duckAmountPercent=8.0, duckReleaseMs=1800.0, outputDb=-5.2), 0.96),
}


def delay(preset='Wide Tape Echo', **changes):
    """An Echoform delay module from one of the app's presets, with changed keys."""
    settings, level = _DELAYS[preset]
    return dict(_DELAY, **{**settings, **changes}, _return=level)


def reverb(preset='Warm Chamber', **changes):
    """An Aetherform reverb module from one of the app's presets, with changed keys."""
    settings, level = _REVERBS[preset]
    return dict(_REVERB, **{**settings, **changes}, _return=level)

# Control ranges

The application and the tools reject a setting outside the range of its
control. Obey these rules in each fix, proposal, and brief:

- Give each value inside its range. A value is in the unit of its control. It
  is not a percentage of the range, unless the unit is percent.
- Quote a current setting with the name of its control and its value from the
  song file, for example "drive amount 18, character 25, mix 50".
- If a control is at its limit, say so, and propose a different control.
- A lane has one EQ, one drive, one compressor, and one ducker. The EQ has a
  low cut, a low shelf, two peaks, a high shelf, and a high cut. Do not propose
  more bands or a second unit.

Lane and clip controls:

| Control | Song key | Range | Default |
| --- | --- | --- | --- |
| Lane fader | lane `gain`, an amplitude | up to +12 dB (amplitude 0 to 4) | 0 dB |
| Lane pan | `pan` | -1 (left) to 1 (right) | 0 |
| Lane sends 1 to 4 | `sends` | 0 to 1 | 0 |
| Lane width | `width` | 0 to 200 % (0 is mono) | 100 |
| Clip gain | clip `gain`, an amplitude | up to +6 dB (amplitude 0 to 2) | 0 dB |
| Clip pitch | `pitchCents` | -2400 to 2400 cents | 0 |
| Clip fades | `fades` | the fade in and the fade out together fit in the clip | 0 ticks |

Lane EQ (`eq`):

| Control | Song key | Range | Default |
| --- | --- | --- | --- |
| Low cut | `lowCut` | 20 to 2000 Hz (20 is off) | 20 |
| Low shelf frequency | `lowHz` | 40 to 500 Hz | 100 |
| Mid peak frequency | `midHz` | 150 to 10,000 Hz | 1000 |
| Second peak frequency | `peak2Hz` | 40 to 16,000 Hz | 250 |
| High shelf frequency | `highHz` | 1500 to 16,000 Hz | 8000 |
| High cut | `highCut` | 1000 to 20,000 Hz (20,000 is off) | 20,000 |
| Band gains | `lowDb`, `midDb`, `peak2Db`, `highDb` | -18 to 18 dB | 0 |
| Peak Q | `midQ`, `peak2Q` | 0.3 to 8 | 1 |

Lane drive (`drive`), compressor (`compressor`), and ducker (`ducker`):

| Control | Song key | Range | Default |
| --- | --- | --- | --- |
| Drive amount | `amount` | 0 to 24 dB | 12 |
| Drive character | `character` | 0 to 100 % | 25 |
| Drive mix | `mix` | 0 to 100 % | 50 |
| Drive focus | `focus` | 20 to 2000 Hz | 100 |
| Drive output | `output` | -12 to 12 dB | 0 |
| Compressor threshold | `threshold` | -60 to 0 dBFS | -20 |
| Compressor ratio | `ratio` | 1 to 20 | 4 |
| Compressor attack | `attack` | 0.1 to 100 ms | 10 |
| Compressor release | `release` | 10 to 1000 ms | 100 |
| Compressor makeup | `makeup` | 0 to 24 dB | 0 |
| Ducker threshold | `threshold` | -60 to 0 dBFS | -30 |
| Ducker depth | `depth` | 0 to 24 dB | 6 |
| Ducker attack | `attack` | 0.1 to 50 ms | 2 |
| Ducker release | `release` | 10 to 1000 ms | 120 |

Automation lanes, FX buses, and the master:

| Control | Range |
| --- | --- |
| Volume lane | -60 dB (silence) to +6 dB |
| Filter lane, pan lane | -1 to 1 |
| Send lanes 1 to 4 | 0 to 1 |
| Repeat lane | 0 (off), 4, 8, 16, 32, or 64 |
| Repeat gate lane | 5 to 100 % |
| Pitch lane | -4800 to 2400 cents |
| FX return level (`returnLevel`) | 0 to 1 |
| FX return lane | 0 to 2 (1 is the full return) |
| Master gain (`masterGain`), an amplitude | 0 to 4 |

This section does not give the ranges of the master pedals and the FX modules.
The application rejects a song with a pedal or module setting outside its
range, so test each such setting with a render.

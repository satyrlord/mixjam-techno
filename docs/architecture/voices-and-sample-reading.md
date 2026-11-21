# Voices and sample reading

## Voices, reversal, and the Pitch lane

`Frames::read_block` with a sends buffer gives the dry sum and the feeds of the
four FX buses, eight interleaved channels for each frame. Each voice adds its
sample, after the gains and pan of the track, multiplied by the send of the
track. A voice applies the fades of its clip for each frame.

A reversed clip's voice reads, for each output frame, the source position of
the mirror frame in the clip (`start + end - 1 - frame`). Thus it plays the
forward slice backwards, as a pure function of the frame and never of what
played. Its fades, its gain, and its pitched copy apply in the time of the
clip, and its sound ends with the clip. The reversal is in the voice, so it
comes before each track stage, the Repeat lane included.

The Pitch lane is also in the voice. `automation::Varispeed` turns the lane of
the track into a clock. The clock is the integral of the speed
`2^(cents / 1200)` over the output frames. Each straight segment in cents has
a closed form. A voice of that track reads its source at the distance of the
clock from the start of the clip, multiplied by the tempo step. A reversed
voice reads back from the last frame of the clip. Thus the read moves at the
speed of the lane without a jump. It is still a pure function of the frame.

Preparation finds where the clock passes the end of the source. It ends the
sound and the fade out of the clip there. The lane scales the read rate of the
pitched copy of the clip, so the pitch of the clip and the lane add in cents.
The lane comes before the Repeat lane, which replays the slowed sound.

## Band-limited sample reading

A voice reads its source between frames when it plays off the sample rate or the
tempo of its source, or under a Pitch lane. `src/audio/resample.rs` reads there
band-limited, with a Kaiser-windowed sinc kernel. Its beta is 8.5, its cutoff is
0.49 of the source rate, and it has 48 taps at speeds of 1 or less. The kernel
is one table of 128 points for each source frame, from its centre to its edge.
Each point holds the step to the next point, so a tap is a straight-line read of
the kernel. The table position of each tap moves outward from the read in fixed
point.

At a speed above 1, the cutoff falls by the speed and the span widens by the
same factor. Thus the read removes the content above the Nyquist frequency of
the output before it can fold back. The gain uses that same rounded tap
spacing. Tap sums use double precision to keep steady levels during long
reads. Preparation rejects a speed when its tap spacing rounds to zero. This
includes the maximum saved Pitch value. Very high speeds that the format can
hold can still need much CPU. The spectral and cost measurements at usual
rates do not cover those workloads.

The spectral tests cover 44.1 kHz and 48 kHz sources at steps 1.047, 1.088,
0.9, and 1.14. Test tones that the kernel passes up to 20 kHz at 44.1 kHz leave
residual images and aliases at least 80 dB down. The gain is within 0.01 dB up
to 17 kHz. These bounds do not cover the transition band near the source Nyquist
frequency, all source rates, or all playback speeds. When playback slows
near-Nyquist content, its attenuation and its residual images can move into
the audible band. WAVs with a low rate can show this at lower frequencies.

The kernel is symmetric and centred on the read, so it adds no delay. Each
frame reads the same position as the straight-line read, and a clip that
starts at frame N sounds at N. The read uses only the slice of the source that
the clip plays, from its offset. Before the slice, it holds the first frame of
the slice. After the source, it holds the last frame of the source, as the
straight-line read does. The start, trim, end, and fades of the clip keep their
frames, and no read goes past the source.

Under a Pitch lane, the cutoff follows the mean speed of the lane over the 32
frames on each side of the frame. That mean is a function of the frame only.
Thus a jump in speed moves the cutoff over 64 frames, not at once. Playback
from any frame and in any block size reads the same as export. A reversed
voice reads the same positions backwards, so it is still a pure function of
the source. A voice at the rate and tempo of its source, without a Pitch lane,
reads whole frames and copies them. `Mix::new` builds the table with
`resample::prepare` before playback reaches the audio thread. The audio thread
only reads the table.

The ignored test `sample_read_cost` measures the cost of one voice for each
read type. Run it in release on a quiet machine. Its result does not establish
a deadline for a full song or a physical device.

Live playback can use the straight-line read when the Sample playback quality
option is Low. Exports, track exports, and song-tool renders prepare at
`PlaybackQuality::FULL` and always read band-limited.

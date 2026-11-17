"""Integrated source time for the native Pitch automation lane."""
import math
from bisect import bisect_right

from mixjam import TICKS_PER_BAR


class PitchClock:
    """The native Pitch lane's integrated speed, in 44.1 kHz output frames."""

    def __init__(self, points, bpm):
        self.frames_per_tick = 44100 * 240 / bpm / TICKS_PER_BAR
        self.frames = [self.frame(point['tick']) for point in points]
        self.cents = [point['value'] for point in points]
        self.times = [self.frames[0]]
        for index in range(1, len(points)):
            length = self.frames[index] - self.frames[index - 1]
            self.times.append(self.times[-1] + self.segment(index - 1, length))

    def frame(self, tick):
        # Match Rust f64::round for the nonnegative timeline.
        return math.floor(tick * self.frames_per_tick + 0.5)

    def segment(self, index, along):
        speed = 2 ** (self.cents[index] / 1200)
        if index + 1 == len(self.frames):
            return speed * along
        length = self.frames[index + 1] - self.frames[index]
        rate = (self.cents[index + 1] - self.cents[index]) / length * math.log(2) / 1200
        return speed * along if abs(rate) < 1e-15 else speed * math.expm1(rate * along) / rate

    def at(self, frame):
        index = bisect_right(self.frames, frame) - 1
        return frame if index < 0 else self.times[index] + self.segment(index, frame - self.frames[index])

    def source_ticks(self, placement, sound, bpm):
        """How long the available source sounds, with reversal anchored at the clip end."""
        start = self.frame(placement['startTick'])
        end = self.frame(placement['startTick'] + placement['durationTicks'])
        step = sound.rate / 44100 * bpm / sound.play_bpm
        distance = max(0, sound.frames - placement['offsetFrames']) / step
        reverse = placement.get('reverse', False)
        origin = self.at(end - 1 if reverse else start)
        low = start
        # The admitted Pitch range never runs slower than 1/16 speed.
        high = end - 1 if reverse else start + math.ceil(distance * 16) + 1
        while low < high:
            middle = (low + high) // 2
            passed = origin - self.at(middle) < distance if reverse else self.at(middle) - origin >= distance
            if passed:
                high = middle
            else:
                low = middle + 1
        return (end - low if reverse else low - start) / self.frames_per_tick

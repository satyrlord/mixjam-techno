"""The bit-level WAV decoding that the song tools share."""
import numpy as np


def unpack_pcm24(raw):
    """The signed integer samples (int32) of little-endian 24-bit PCM bytes."""
    b = np.frombuffer(raw, np.uint8).reshape(-1, 3).astype(np.int32)
    ints = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
    return np.where(ints >= 1 << 23, ints - (1 << 24), ints)

"""Read the private settings of the song tools from `.env.local` at the repository root.

`.env.local` holds one `KEY="value"` per line (quotes optional; `#` starts a comment
line). An environment variable of the same name overrides the file. A relative path
value is read from the repository root. `.env.example` lists every key.
"""
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ENV_FILE = '.env.local'


def parse(text):
    """The KEY="value" pairs of an env file's text."""
    values = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith('#') or '=' not in line:
            continue
        key, value = line.split('=', 1)
        key, value = key.strip(), value.strip()
        if key.startswith('export '):
            key = key[len('export '):].strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in '"\'':
            value = value[1:-1]
        values[key] = value
    return values


def load(root=ROOT):
    """The file's values with the environment's overrides."""
    path = Path(root) / ENV_FILE
    values = parse(path.read_text(encoding='utf-8')) if path.exists() else {}
    for key in list(values):
        if key in os.environ:
            values[key] = os.environ[key]
    return values


def get(key, root=ROOT):
    """The value of `key`, or None when neither the environment nor `.env.local` sets it."""
    if os.environ.get(key):
        return os.environ[key]
    return load(root).get(key) or None


def require(key, root=ROOT):
    """The value of `key`; stop with a clear message when it is missing."""
    value = get(key, root)
    if value is None:
        raise SystemExit(f'{key} is not set. Add {key}="..." to {ENV_FILE} at the repository root '
                         f'({Path(root)}) or set it in the environment. .env.example describes it.')
    return value


def path(key, required=True, root=ROOT):
    """The value of `key` as a path; a relative value is relative to the repository root."""
    value = require(key, root) if required else get(key, root)
    if value is None:
        return None
    value = Path(value)
    return value if value.is_absolute() else Path(root) / value


SAMPLE_LIBRARIES = 9


def sample_folders(root=ROOT):
    """The sample libraries SAMPLES_FOLDER_1 to SAMPLES_FOLDER_9 that are set, in number order."""
    keys = [f'SAMPLES_FOLDER_{n}' for n in range(1, SAMPLE_LIBRARIES + 1)]
    return [found for key in keys if (found := path(key, required=False, root=root)) is not None]


def exe(root=ROOT):
    """The release build of the application, which the tools use to render."""
    name = 'mixjam-techno.exe' if sys.platform == 'win32' else 'mixjam-techno'
    found = Path(root) / 'target' / 'release' / name
    if not found.exists():
        raise SystemExit(f'{found} does not exist. Build it with the release command in README.md.')
    return found


def reference_profile(root=ROOT):
    """The reference profile path: REFERENCE_PROFILE, else reference.json beside ARTIST_PROFILE, else None."""
    explicit = path('REFERENCE_PROFILE', required=False, root=root)
    if explicit is not None:
        return explicit
    artist = path('ARTIST_PROFILE', required=False, root=root)
    return artist.parent / 'reference.json' if artist is not None else None

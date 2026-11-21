"""Option lookup for the song tools that read their own command line.

A flag needs a value before the next flag. These helpers report a missing value.
"""


def _value(args, position, convert):
    name = args[position]
    if position + 1 >= len(args) or args[position + 1].startswith('--'):
        raise SystemExit(f'{name} needs a value.')
    try:
        return convert(args[position + 1])
    except ValueError:
        raise SystemExit(f'{name} cannot use {args[position + 1]!r}.') from None


def option(args, name, convert=str, default=None):
    """The value after the first `name` in `args`, converted; `default` when `name` is not there."""
    return _value(args, args.index(name), convert) if name in args else default


def options(args, name, convert=str):
    """The values after every `name` in `args`, converted, in order."""
    return [_value(args, position, convert) for position, word in enumerate(args) if word == name]


def pop_option(args, name, convert=str, default=None):
    """Like `option`, and remove the flag and its value from `args`."""
    value = option(args, name, convert, default)
    if name in args:
        position = args.index(name)
        del args[position:position + 2]
    return value

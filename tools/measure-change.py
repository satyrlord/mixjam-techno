"""Measure a change: complexity and duplication.

Usage, from the repository root:

    python tools/measure-change.py [--base REV] [--lcov FILE]

The script compares the worktree, untracked Rust files included, with REV
(default HEAD). It reports two results for the changed lines:

- The cyclomatic complexity of each changed production function. With
  --lcov, report the CRAP score instead. CRAP is
  complexity^2 * (1 - coverage)^3 + complexity, with the cyclomatic complexity
  of rust-code-analysis-cli and the line coverage of the cargo-llvm-cov LCOV
  file. A score of 10 or more is a finding.
- Each jscpd clone of which one copy has a changed line. Each clone is a
  finding.

Only the files in src/ are measured. Test code is not measured: files under a
tests folder, files named tests.rs or *_tests.rs, the files of external
#[cfg(test)] modules and their child modules, and inline #[cfg(test)] items.
The exit code is 1 if there is a finding, Git cannot read the change, or a
required tool or the requested coverage file is missing. Otherwise it is 0.
"""
import argparse
import codecs
from dataclasses import dataclass
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'tmp' / 'measure'
CRAP_LIMIT = 10
HUNK = re.compile(r'^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@')
CFG_TEST = re.compile(r'^(\s*)#\[cfg\(test\)\]\s*(.*)$')
ATTRIBUTE = re.compile(r'^\s*(#\[.*\]|//.*)\s*$')
MODULE = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*([;{])')
PATH_ATTRIBUTE = re.compile(r'#\[path\s*=\s*"([^"]+)"\]')
# Fixed diff options, so that the git configuration of the user cannot change the format.
DIFF_OPTIONS = ('--no-color', '--no-ext-diff', '--no-textconv', '--src-prefix=a/', '--dst-prefix=b/')


@dataclass
class Function:
    name: str
    start: int
    end: int
    complexity: float


@dataclass
class CrapRow:
    path: str
    name: str
    start: int
    complexity: float
    coverage: float | None
    crap: float | None


@dataclass
class Clone:
    first: tuple
    second: tuple
    lines: int
    tokens: int


def changed_lines(diff):
    """The added lines on the new side of a unified diff, by repository path."""
    changed, path, line = {}, None, 0
    for text in diff.splitlines():
        if text.startswith('+++ '):
            target = text[4:].rstrip('\t\r\n')
            if target.startswith('"') and target.endswith('"'):
                target = codecs.escape_decode(target[1:-1].encode('utf-8'))[0].decode('utf-8')
            path = target[2:] if target.startswith('b/') else None
            continue
        if text.startswith('--- ') or path is None:
            continue
        match = HUNK.match(text)
        if match:
            line = int(match.group(1))
        elif text.startswith('+'):
            changed.setdefault(path, set()).add(line)
            line += 1
        elif text.startswith(' '):
            line += 1
    return changed


def repository_path(path, root):
    """A path as a repository-relative POSIX string."""
    absolute = Path(path)
    if not absolute.is_absolute():
        absolute = root / absolute
    return PurePosixPath(Path(os.path.relpath(absolute, root))).as_posix()


def parse_lcov(text, root):
    """Execution counts by line, by repository path."""
    coverage, current = {}, None
    for line in text.splitlines():
        if line.startswith('SF:'):
            current = coverage.setdefault(repository_path(line[3:], root), {})
        elif line.startswith('DA:') and current is not None:
            number, count = line[3:].split(',')[:2]
            current[int(number)] = int(count)
        elif line == 'end_of_record':
            current = None
    return coverage


def functions(space):
    """The named functions of a rust-code-analysis space. Closures stay in the
    complexity of the function that holds them."""
    found = []
    for child in space['spaces']:
        if child['kind'] == 'function' and child['name'] != '<anonymous>':
            found.append(Function(child['name'], child['start_line'], child['end_line'],
                                  float(child['metrics']['cyclomatic']['sum'])))
        if child['kind'] != 'function':
            found.extend(functions(child))
    return found


def cfg_test_items(lines):
    """For each #[cfg(test)] item: the 0-based attribute line, the 0-based item
    line, the attribute indent, the item text, and the attribute lines."""
    for index, text in enumerate(lines):
        match = CFG_TEST.match(text)
        if not match:
            continue
        if match.group(2):
            yield index, index, match.group(1), match.group(2), [text]
            continue
        item = index + 1
        while item < len(lines) and ATTRIBUTE.match(lines[item]):
            item += 1
        if item < len(lines):
            yield index, item, match.group(1), lines[item], lines[index:item]


def item_end(lines, item, indent):
    """The 0-based last line of the item that starts at `item`, or None."""
    for line in range(item, len(lines)):
        text = lines[line]
        if '{' in text:
            if text.count('{') == text.count('}'):
                return line
            closings = (indent + '}', indent + '};')
            return next((end for end in range(line + 1, len(lines)) if lines[end] in closings), None)
        if text.rstrip().endswith(';'):
            return line
    return None


def test_ranges(source):
    """The 1-based line ranges of the inline #[cfg(test)] items. An external
    module declaration (`mod x;`) has no range: its file is test code."""
    lines = source.splitlines()
    ranges = []
    for attribute, item, indent, text, _ in cfg_test_items(lines):
        module = MODULE.match(text)
        if module and module.group(2) == ';':
            continue
        end = item_end(lines, item, indent)
        if end is not None:
            ranges.append((attribute + 1, end + 1))
    return ranges


def module_folder(path):
    """The folder of the child modules of a module file."""
    path = PurePosixPath(path)
    return path.parent if path.name in ('mod.rs', 'lib.rs', 'main.rs') else path.with_suffix('')


def test_module_files(sources):
    """The repository paths of the files of the external #[cfg(test)] modules
    that the given sources declare, by repository path."""
    files = set()
    for path, source in sources.items():
        for _, _, _, text, attributes in cfg_test_items(source.splitlines()):
            module = MODULE.match(text)
            if not module or module.group(2) != ';':
                continue
            explicit = [match.group(1) for match in map(PATH_ATTRIBUTE.search, attributes) if match]
            if explicit:
                files.add((PurePosixPath(path).parent / explicit[0]).as_posix())
            else:
                folder = module_folder(path) / module.group(1)
                files.update({f'{folder}.rs', f'{folder}/mod.rs'})
    return files


def is_test_path(path, test_files=frozenset()):
    """Whether `path` is test code by its name, or is an external test module
    of `test_files` or one of its child modules."""
    parts = PurePosixPath(path).parts
    name = parts[-1]
    if 'tests' in parts[:-1] or name == 'tests.rs' or name.endswith('_tests.rs'):
        return True
    return any(path == file or path.startswith(f'{module_folder(file)}/') for file in test_files)


def in_ranges(start, end, ranges):
    return any(start <= high and low <= end for low, high in ranges)


def crap(complexity, coverage):
    return complexity ** 2 * (1 - coverage) ** 3 + complexity


def crap_rows(path, unit, source, coverage, changed):
    """The complexity / CRAP rows of the production functions of `path` that hold a changed line."""
    tests = test_ranges(source)
    lines = coverage.get(path, {}) if coverage is not None else None
    rows = []
    for function in functions(unit):
        span = range(function.start, function.end + 1)
        if in_ranges(function.start, function.end, tests) or not changed.intersection(span):
            continue
        if lines is None:
            rows.append(CrapRow(path, function.name, function.start, function.complexity, None, None))
            continue
        measured = [lines[line] for line in span if line in lines]
        fraction = sum(1 for count in measured if count > 0) / len(measured) if measured else None
        score = None if fraction is None else crap(function.complexity, fraction)
        rows.append(CrapRow(path, function.name, function.start, function.complexity, fraction, score))
    return rows


def clones_touching(report, scan_root, changed, source_of, test_files=frozenset()):
    """The jscpd clones of production code of which one copy has a changed line."""
    found = []
    for clone in report['duplicates']:
        sides = []
        for key in ('firstFile', 'secondFile'):
            side = clone[key]
            path = PurePosixPath(scan_root, *re.split(r'[\\/]', side['name'])).as_posix()
            sides.append((path, side['start'], side['end']))

        def is_test(side):
            source = source_of(side[0]) or ''
            return is_test_path(side[0], test_files) or in_ranges(side[1], side[2], test_ranges(source))

        def touches(side):
            return bool(changed.get(side[0], set()).intersection(range(side[1], side[2] + 1)))

        if not any(map(is_test, sides)) and any(map(touches, sides)):
            found.append(Clone(sides[0], sides[1], clone['lines'], clone['tokens']))
    return found


def git(root, *arguments, expected_status=(0,)):
    result = subprocess.run(['git', '-C', str(root), '-c', 'core.quotePath=false', *arguments],
                            capture_output=True, text=True, encoding='utf-8', check=False)
    if result.returncode not in expected_status:
        command = ' '.join(arguments)
        raise ValueError(f'Git {command} failed. {result.stderr.strip()}')
    return result.stdout


def change_diff(base, root=None):
    """The diff of the worktree against `base`, with untracked Rust files as new files."""
    root = root or ROOT
    tree = git(root, 'rev-parse', '--verify', '--end-of-options', f'{base}^{{tree}}').strip()
    diff = git(root, 'diff', *DIFF_OPTIONS, tree, '--', '*.rs')
    untracked = git(root, 'ls-files', '-z', '--others', '--exclude-standard', '--', '*.rs')
    for path in filter(None, untracked.split('\0')):
        diff += git(root, 'diff', *DIFF_OPTIONS, '--no-index', '--', '/dev/null', path,
                    expected_status=(0, 1))
    return diff


def stale_files(lcov, root, paths):
    """The paths that changed after the coverage file was written."""
    written = lcov.stat().st_mtime
    return [path for path in paths if (root / path).stat().st_mtime > written]


def tool(name):
    local = ROOT / 'local-data' / 'toolchain' / 'cargo' / 'bin' / f'{name}.exe'
    return str(local) if local.is_file() else shutil.which(name)


def complexity_units(paths):
    """rust-code-analysis-cli output for each path."""
    analyser = tool('rust-code-analysis-cli')
    if analyser is None:
        raise SystemExit('rust-code-analysis-cli is missing: run tools/runtime/install-quality-tools.ps1.')
    units = {}
    for path in paths:
        result = subprocess.run([analyser, '-m', '-O', 'json', '-p', str(ROOT / path)],
                                capture_output=True, text=True, encoding='utf-8', check=True)
        units[path] = json.loads(result.stdout)
    return units


def duplication_report():
    jscpd = shutil.which('jscpd', path=str(ROOT / 'node_modules' / '.bin'))
    if jscpd is None:
        raise SystemExit('jscpd is missing: run npm ci.')
    folder = OUTPUT / 'jscpd'
    subprocess.run([jscpd, '--format', 'rust', '--min-tokens', '50', '--reporters', 'json',
                    '--output', str(folder), '--silent', 'src'], cwd=ROOT, check=True,
                   capture_output=True)
    return json.loads((folder / 'jscpd-report.json').read_text(encoding='utf-8'))


def report_scores(rows, with_coverage):
    findings = 0
    header = 'CRAP scores' if with_coverage else 'Complexity scores (coverage skipped; pass --lcov for CRAP)'
    print(f'\n{header} of {len(rows)} changed functions (limit {CRAP_LIMIT}):')
    for row in sorted(rows, key=lambda r: -(r.crap if r.crap is not None else r.complexity)):
        if with_coverage:
            if row.crap is None:
                print(f'  not measured  {row.path}:{row.start} {row.name} (no instrumented lines)')
                continue
            failed = row.crap >= CRAP_LIMIT
            findings += failed
            mark = 'FINDING' if failed else 'ok'
            print(f'  {mark:7} {row.crap:6.1f}  {row.path}:{row.start} {row.name} '
                  f'(complexity {row.complexity:.0f}, coverage {row.coverage:.0%})')
        else:
            failed = row.complexity >= CRAP_LIMIT
            findings += failed
            mark = 'FINDING' if failed else 'ok'
            print(f'  {mark:7} {row.complexity:6.0f}  {row.path}:{row.start} {row.name}')
    return findings


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--base', default='HEAD', help='the revision to compare with')
    parser.add_argument('--lcov', default=None, help='optional cargo-llvm-cov LCOV file for full CRAP scoring')
    options = parser.parse_args(arguments)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    try:
        diff = change_diff(options.base)
    except ValueError as error:
        print(error)
        return 1
    everything = {path.relative_to(ROOT).as_posix(): path.read_text(encoding='utf-8')
                  for path in (ROOT / 'src').rglob('*.rs')}
    test_files = test_module_files(everything)
    changed = {path: lines for path, lines in changed_lines(diff).items()
               if path.startswith('src/') and not is_test_path(path, test_files)}
    print(f'Changed production Rust files: {len(changed)}.')
    if not changed:
        print('\nFindings: 0')
        return 0
    coverage = None
    if options.lcov is not None:
        lcov = Path(options.lcov)
        if not lcov.is_file():
            print(f'No coverage file at {lcov}. Run cargo llvm-cov first (only when explicitly ordered).')
            return 1
        stale = stale_files(lcov, ROOT, changed)
        if stale:
            print(f'WARNING: {len(stale)} changed files are newer than the coverage file, so the coverage '
                  f'and the line numbers can be wrong. Run the coverage command again. {", ".join(stale)}')
        coverage = parse_lcov(lcov.read_text(encoding='utf-8'), ROOT)
    sources = {path: (ROOT / path).read_text(encoding='utf-8') for path in changed}
    rows = [row for path, unit in complexity_units(changed).items()
            for row in crap_rows(path, unit, sources[path], coverage, changed[path])]
    findings = report_scores(rows, coverage is not None)

    def source_of(path):
        file = ROOT / path
        return file.read_text(encoding='utf-8') if file.is_file() else None

    clones = clones_touching(duplication_report(), 'src', changed, source_of, test_files)
    print(f'\nClones that touch the change: {len(clones)}')
    for clone in clones:
        print(f'  FINDING {clone.lines} lines, {clone.tokens} tokens: '
              f'{clone.first[0]}:{clone.first[1]}-{clone.first[2]} = '
              f'{clone.second[0]}:{clone.second[1]}-{clone.second[2]}')
    findings += len(clones)
    print(f'\nFindings: {findings}')
    return 1 if findings else 0


if __name__ == '__main__':
    sys.exit(main())

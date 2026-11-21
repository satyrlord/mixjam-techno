"""Checks for the change measurements: CRAP scores and duplication."""
from contextlib import redirect_stdout
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("measure_change", ROOT / "tools/measure-change.py")
measure = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(measure)

DIFF = """diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -3,2 +3,3 @@ fn x() {
 context
-old
+new one
+new two
@@ -20 +21,0 @@
-gone
diff --git a/src/b.rs b/src/b.rs
deleted file mode 100644
--- a/src/b.rs
+++ /dev/null
@@ -1 +0,0 @@
-all gone
diff --git a/src/c.rs b/src/c.rs
new file mode 100644
--- /dev/null
+++ b/src/c.rs
@@ -0,0 +1,2 @@
+fn c() {}
+fn d() {}
"""


def space(name, start, end, cyclomatic, children=(), kind='function'):
    return {'kind': kind, 'name': name, 'start_line': start, 'end_line': end,
            'metrics': {'cyclomatic': {'sum': cyclomatic}}, 'spaces': list(children)}


class ChangedLinesTests(unittest.TestCase):
    def test_only_added_lines_on_the_new_side_count(self):
        self.assertEqual(measure.changed_lines(DIFF), {'src/a.rs': {4, 5}, 'src/c.rs': {1, 2}})

    def test_paths_with_spaces_and_quotes_are_read(self):
        diff = ('--- /dev/null\n+++ b/src/sp ace.rs\t\n@@ -0,0 +1 @@\n+x\n'
                '--- /dev/null\n+++ "b/src/q\\"uote.rs"\n@@ -0,0 +1 @@\n+y\n')
        self.assertEqual(measure.changed_lines(diff), {'src/sp ace.rs': {1}, 'src/q"uote.rs': {1}})


def run_git(root, *arguments):
    subprocess.run(['git', '-C', str(root), *arguments], check=True, capture_output=True)


class ChangeDiffTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.root = Path(self.folder.name)
        run_git(self.root, 'init', '-q')
        for key, value in (('user.email', 'a@b.c'), ('user.name', 'a'), ('diff.noprefix', 'true'),
                           ('diff.mnemonicPrefix', 'true'), ('color.diff', 'always'),
                           ('core.autocrlf', 'false')):
            run_git(self.root, 'config', key, value)
        (self.root / 'src').mkdir()
        (self.root / 'src' / 'a.rs').write_text('fn a() {}\n', encoding='utf-8')
        run_git(self.root, 'add', '.')
        run_git(self.root, 'commit', '-q', '-m', 'base')

    def tearDown(self):
        self.folder.cleanup()

    def test_user_diff_settings_and_spaces_do_not_hide_changes(self):
        (self.root / 'src' / 'a.rs').write_text('fn a() {}\nfn b() {}\n', encoding='utf-8')
        (self.root / 'src' / 'new file.rs').write_text('fn c() {}\n', encoding='utf-8')
        diff = measure.change_diff('HEAD', self.root)
        self.assertEqual(measure.changed_lines(diff), {'src/a.rs': {2}, 'src/new file.rs': {1}})

    def test_no_change_passes_and_a_change_without_coverage_fails(self):
        output = self.root / 'tmp' / 'measure'
        arguments = ['--lcov', str(output / 'lcov.info')]
        with mock.patch.object(measure, 'ROOT', self.root), mock.patch.object(measure, 'OUTPUT', output), \
                redirect_stdout(io.StringIO()):
            self.assertEqual(measure.main(arguments), 0)
            (self.root / 'src' / 'a.rs').write_text('fn a() {}\nfn b() {}\n', encoding='utf-8')
            self.assertEqual(measure.main(arguments), 1)

    def test_coverage_older_than_a_changed_file_gives_a_warning(self):
        lcov = self.root / 'lcov.info'
        lcov.write_text('', encoding='utf-8')
        source = self.root / 'src' / 'a.rs'
        source.write_text('fn a() {}\nfn b() {}\n', encoding='utf-8')
        os.utime(lcov, (1, 1))
        self.assertEqual(measure.stale_files(lcov, self.root, ['src/a.rs']), ['src/a.rs'])
        os.utime(lcov, None)
        os.utime(source, (1, 1))
        self.assertEqual(measure.stale_files(lcov, self.root, ['src/a.rs']), [])


class CoverageTests(unittest.TestCase):
    def test_lcov_lines_are_keyed_by_the_repository_path(self):
        text = f"SF:{ROOT / 'src' / 'a.rs'}\nDA:4,0\nDA:5,3\nend_of_record\nSF:{ROOT / 'src' / 'c.rs'}\nDA:1,1\nend_of_record\n"
        self.assertEqual(measure.parse_lcov(text, ROOT), {'src/a.rs': {4: 0, 5: 3}, 'src/c.rs': {1: 1}})


class FunctionTests(unittest.TestCase):
    def test_named_functions_carry_their_closures_and_anonymous_ones_are_not_listed(self):
        unit = space('src/a.rs', 1, 40, 9, kind='unit', children=[
            space('Thing', 2, 30, 8, kind='impl', children=[
                space('run', 3, 20, 7, children=[space('<anonymous>', 5, 5, 1)]),
            ]),
            space('helper', 32, 40, 2),
        ])
        self.assertEqual(
            [(f.name, f.start, f.end, f.complexity) for f in measure.functions(unit)],
            [('run', 3, 20, 7.0), ('helper', 32, 40, 2.0)],
        )

    def test_inline_test_modules_and_test_files_are_test_code(self):
        source = "fn a() {}\n\n#[cfg(test)]\nmod tests {\n    fn b() {}\n}\nfn after() {}\n"
        self.assertEqual(measure.test_ranges(source), [(3, 6)])
        for path in ('src/x/tests/y.rs', 'src/x/tests.rs', 'src/ui/tests/z_tests.rs', 'src/a_tests.rs'):
            self.assertTrue(measure.is_test_path(path), path)
        self.assertFalse(measure.is_test_path('src/testing_tools.rs'))

    def test_an_external_test_module_declaration_is_not_a_block(self):
        source = "#[cfg(test)]\nmod tests;\nmod views;\n\npub fn load() {\n    work();\n}\n"
        self.assertEqual(measure.test_ranges(source), [])

    def test_other_test_only_items_are_test_code(self):
        source = (
            "#[cfg(test)]\n"                      # 1
            "#[allow(dead_code)]\n"               # 2
            "pub(crate) mod helpers {\n"          # 3
            "    fn h() {}\n"                     # 4
            "}\n"                                 # 5
            "#[cfg(test)]\n"                      # 6
            "pub(crate) use a::b;\n"              # 7
            "#[cfg(test)]\n"                      # 8
            "use c::{\n"                          # 9
            "    d,\n"                            # 10
            "};\n"                                # 11
            "impl X {\n"                          # 12
            "    #[cfg(test)]\n"                  # 13
            "    pub fn only_tests(&self) -> u8 {\n"  # 14
            "        1\n"                         # 15
            "    }\n"                             # 16
            "    #[cfg(test)]\n"                  # 17
            "    fn tiny() {}\n"                  # 18
            "    pub fn real(&self) {}\n"         # 19
            "}\n"                                 # 20
        )
        self.assertEqual(measure.test_ranges(source), [(1, 5), (6, 7), (8, 11), (13, 16), (17, 18)])

    def test_external_test_modules_and_their_children_are_test_files(self):
        sources = {
            'src/audio.rs': "#[cfg(test)]\nmod realtime_bench;\nmod frames;\n#[cfg(test)]\n#[path = \"audio/x_check.rs\"]\nmod x;\n",
            'src/agent/mod.rs': "#[cfg(test)]\npub(in crate) mod tests;\n",
        }
        files = measure.test_module_files(sources)
        for path in ('src/audio/realtime_bench.rs', 'src/audio/realtime_bench/mod.rs',
                     'src/audio/realtime_bench/deep.rs', 'src/audio/x_check.rs', 'src/agent/tests.rs'):
            self.assertTrue(measure.is_test_path(path, files), path)
        for path in ('src/audio/frames.rs', 'src/audio.rs', 'src/agent/mod.rs'):
            self.assertFalse(measure.is_test_path(path, files), path)


class CrapTests(unittest.TestCase):
    def test_the_formula_rewards_coverage(self):
        self.assertEqual(measure.crap(5, 1.0), 5)
        self.assertEqual(measure.crap(5, 0.0), 30)
        self.assertAlmostEqual(measure.crap(4, 0.5), 4 + 16 * 0.125)

    def test_only_changed_production_functions_are_scored(self):
        unit = space('src/a.rs', 1, 60, 20, kind='unit', children=[
            space('changed', 1, 10, 6),
            space('untouched', 12, 20, 9),
            space('unmeasured', 22, 30, 3),
            space('in_tests', 42, 50, 9),
        ])
        source = '\n' * 39 + '#[cfg(test)]\nmod tests {\n' + '\n' * 18 + '}\n'
        coverage = {line: (1 if line <= 5 else 0) for line in range(1, 11)}
        coverage.update({line: 0 for line in range(12, 21)})
        coverage.update({line: 0 for line in range(42, 51)})
        rows = measure.crap_rows('src/a.rs', unit, source, {'src/a.rs': coverage}, {2, 25, 45})
        self.assertEqual([(row.name, row.coverage) for row in rows], [('changed', 0.5), ('unmeasured', None)])
        self.assertAlmostEqual(rows[0].crap, 6 + 36 * 0.125)
        self.assertIsNone(rows[1].crap)


class DuplicationTests(unittest.TestCase):
    def test_a_clone_counts_when_either_copy_touches_the_change_outside_tests(self):
        def clone(first, second):
            return {'lines': 8, 'tokens': 60,
                    'firstFile': {'name': first[0], 'start': first[1], 'end': first[2]},
                    'secondFile': {'name': second[0], 'start': second[1], 'end': second[2]}}

        report = {'duplicates': [
            clone(('a.rs', 1, 8), ('d.rs', 40, 47)),
            clone(('d.rs', 1, 8), ('a.rs', 30, 37)),
            clone(('d.rs', 1, 8), ('e.rs', 1, 8)),
            clone(('tests\\f.rs', 1, 8), ('a.rs', 2, 9)),
        ]}
        sources = {'src/a.rs': '', 'src/d.rs': '', 'src/e.rs': ''}
        clones = measure.clones_touching(report, 'src', {'src/a.rs': {5}}, sources.get)
        self.assertEqual([(c.first, c.second) for c in clones], [(('src/a.rs', 1, 8), ('src/d.rs', 40, 47))])


if __name__ == '__main__':
    unittest.main()

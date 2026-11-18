import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
RUST = ROOT / 'packages/core/src/build_support.rs'
TOOL = ROOT / 'native/tools/supported_builds.py'


def generated(source):
    with tempfile.TemporaryDirectory() as tmp:
        src = Path(tmp) / 'build_support.rs'
        out = Path(tmp) / 'supported_builds.h'
        src.write_text(source)
        subprocess.run([sys.executable, str(TOOL), str(src), str(out)], check=True)
        return out.read_text()


def rows(header):
    return [(int(a), int(b)) for a, b in re.findall(r'\{(\d+)ULL, (\d+)UL\}', header)]


class SupportedBuildsTest(unittest.TestCase):
    def test_the_header_lists_every_build_the_launcher_supports(self):
        source = RUST.read_text()
        block = source.split('const SUPPORTED: &[(u64, u32)] = &[', 1)[1].split('];', 1)[0]
        named = {n: (int(a.replace('_', '')), int(b.replace('_', '')))
                 for n, a, b in re.findall(r'pub const (BUILD_\w+): \(u64, u32\) = \(([\d_]+), ([\d_]+)\);', source)}
        expected = []
        for line in block.splitlines():
            line = line.strip().rstrip(',')
            if line in named:
                expected.append(named[line])
            elif line:
                a, b = re.fullmatch(r'\(([\d_]+), ([\d_]+)\)', line).groups()
                expected.append((int(a.replace('_', '')), int(b.replace('_', ''))))
        self.assertTrue(expected)
        self.assertEqual(rows(generated(source)), expected)

    def test_a_named_constant_and_a_literal_both_read(self):
        source = ('pub const BUILD_A: (u64, u32) = (1_000, 2_000);\n'
                  'const SUPPORTED: &[(u64, u32)] = &[\n    (3_000, 4_000),\n    BUILD_A,\n];\n')
        self.assertEqual(rows(generated(source)), [(3000, 4000), (1000, 2000)])

    def test_an_entry_it_cannot_read_stops_the_build(self):
        source = 'const SUPPORTED: &[(u64, u32)] = &[\n    something_else(),\n];\n'
        with self.assertRaises(subprocess.CalledProcessError):
            generated(source)


if __name__ == '__main__':
    unittest.main()

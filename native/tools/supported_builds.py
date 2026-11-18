import re
import sys

source = open(sys.argv[1]).read()
named = {name: (a, b) for name, a, b in re.findall(r'pub const (BUILD_\w+): \(u64, u32\) = \(([\d_]+), ([\d_]+)\);', source)}
block = source.split('const SUPPORTED: &[(u64, u32)] = &[', 1)[1].split('];', 1)[0]
builds = []
for line in block.splitlines():
    line = line.strip().rstrip(',')
    if not line:
        continue
    if line in named:
        builds.append(named[line])
        continue
    m = re.fullmatch(r'\(([\d_]+), ([\d_]+)\)', line)
    if not m:
        sys.exit(f'supported_builds.py: cannot read the SUPPORTED entry {line!r}')
    builds.append((m.group(1), m.group(2)))
rows = ''.join(f'    {{{a.replace("_", "")}ULL, {b.replace("_", "")}UL}},\n' for a, b in builds)
open(sys.argv[2], 'w').write('static const struct {\n    unsigned long long size;\n    unsigned long stamp;\n} '
                              'SWOFTY_SUPPORTED_BUILDS[] = {\n' + rows + '};\n')

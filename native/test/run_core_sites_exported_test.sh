#!/usr/bin/env bash
# Host-only. Every "site_*" name in native/src/core that the core resolves by name, and every name in the live
# state's SITE_NAMES table (live_state.cpp), must be exported in native/include/swofty/game/*.hpp, or sit in
# native/test/core_sites_unexported.txt with a reason. A name written as Sig{"site_X", "<byte pattern>", ...} is
# found by signature scan and needs no export.
set -euo pipefail
cd "$(dirname "$0")/../.."
python3 - <<'PY'
import glob, re, sys

allow = {}
for line in open('native/test/core_sites_unexported.txt'):
    if line.strip() and not line.startswith('#'):
        name, _, why = line.rstrip('\n').partition('\t')
        if not why.strip():
            sys.exit(f'core_sites_unexported.txt: {name} has no reason')
        allow[name] = why

exported, bound = set(), set()
for f in glob.glob('native/include/swofty/game/*.hpp'):
    text = open(f).read()
    exported |= set(re.findall(r'<"(\w+)"', text))
    bound |= set(re.findall(r'^inline constexpr address<"(\w+)">', text, re.M))

named, scanned = {}, set()
for f in glob.glob('native/src/core/**/*.[ch]pp', recursive=True):
    text = open(f).read()
    for m in re.finditer(r'"(site_\w+)"', text):
        named.setdefault(m.group(1), set()).add(f)
    for m in re.finditer(r'\{"(\w+)",\s*&\w+::Sites::', text):
        named.setdefault(m.group(1), set()).add(f)
    scanned |= set(re.findall(r'"(site_\w+)"\s*,\s*"(?:[0-9A-Fa-f?]{2}\s*)+', text))

targets = set()
for f in glob.glob('native/src/core/**/*.[ch]pp', recursive=True):
    targets |= set(re.findall(r'\w+_TARGET\s*=\s*"(\w+)"', open(f).read()))

bad = [f'{n} ({", ".join(sorted(fs))}): not exported and resolved by name' for n, fs in sorted(named.items())
       if n not in exported and n not in scanned and n not in allow]
bad += [f'{n}: in the allowlist but {"exported now" if n in exported else "no longer referenced" if n not in named else "found by signature scan"}; remove it'
        for n in sorted(allow) if n in exported or n not in named or n in scanned]
bad += [f'{n}: a *_TARGET the core resolves by name is not an exported binding' for n in sorted(targets - bound)]
if bad:
    print('\n'.join(bad))
    sys.exit(1)
print(f'core sites: {len(named)} names, {len(allow)} allowlisted, all others exported or found by signature scan; {len(targets)} named target(s) exported')
PY

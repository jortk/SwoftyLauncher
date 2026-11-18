#!/usr/bin/env python3
"""Turn a launch's loader.log into binding coverage, an override worklist and verified hints.

  capture.py <loader.log | session dir | Send Logs zip> [...] [--build steam|ea] [--json] [--write]

Reads the per-binding report (`[bind] id=... result=...` and `[bind] summary ...`, the core's log contract) and the
older lines (`[core] bind X -> 0x...`, `[core] binding X: unresolved (...)`, `[ea] binding X ... VERDICT ...`). The
latest pass per binding wins. Hints come only from verified pattern resolutions; an older line that names just the
resolved target is listed as target-only and never becomes a hint. `--write` merges the hints into
re/db/builds/<build>.json after checking the log's build identity against that file's `build` block.
"""
import argparse, difflib, hashlib, json, os, re, sys, zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
DB = os.path.join(os.path.dirname(HERE), 'db')
REPO = os.path.dirname(os.path.dirname(HERE))

TS_RE = re.compile(r'^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d(?:\.\d+)?)')
ATTACH_RE = re.compile(r'swofty loader .*attached, pid (\d+)')
EXE_RE = re.compile(r'\bexe: size (\d+) \(analysed \d+\), PE timestamp (-?\d+)')
SELECT_RE = re.compile(r'\[core\] bindings: \d+ set\(s\) present, (?:selected (\S+) by PE timestamp (-?\d+)'
                       r'|none match PE timestamp (-?\d+)(?:; using (\S+)| and no bindings\.json; using (\S+)))')
SET_RE = re.compile(r'\[core\] (\S+\.json): (\d+) binding\(s\) for build (\S+), (this exe|a different exe)')
BIND_RE = re.compile(r'\[bind\] (.*?)\s*$')
KV_RE = re.compile(r'([A-Za-z_]\w*)=("(?:[^"\\]|\\.)*"|\S*)')
CORE_BIND_RE = re.compile(r'\[core\] bind (\S+) -> 0x([0-9a-fA-F]+) \(base\+0x([0-9a-fA-F]+)\), (\S+) (\S+) '
                          r'via (.+?)\s*$')
CORE_WHY_RE = re.compile(r'\[core\] binding (\S+): (.+?)\s*$')
CORE_MISSING_RE = re.compile(r'\[core\] no binding (\S+) in (\S+)')
STAGE_RE = re.compile(r'\] stage: ([a-z]\w*) \(')
EA_REPORT_RE = re.compile(r'\[ea\] per-binding resolution report \(([^)]+)\)')
EA_BIND_RE = re.compile(r'\[ea\] binding (\S+)\s+(.*?)VERDICT (\S+)(.*)$')
EA_SITE_RE = re.compile(r'\bsite @base\+0x([0-9a-fA-F]+)')
EA_FAIL_RE = re.compile(r'^ \((.*)\); candidate site @base\+0x([0-9a-fA-F]+) bytes \[([^\]]*)\]')

LEGACY_METHOD = {'pattern at the KB address': 'hint', 'pattern': 'scan', 'string anchor': 'anchor',
                 'string anchor (pattern fell back)': 'anchor-fallback'}
# Reasons manager.cpp passes to `unresolved()`; other `[core] binding X: ...` lines are progress notes.
LEGACY_WHY = {'anchor offset is not mapped on this build', 'anchor is not the rel32 of a call or jump',
              'anchor is not inside the RIP-relative instruction', 'unknown resolve kind', 'expect bytes differ',
              'target is not in read-only code', 'target is outside the exe', 'target differs from the KB build',
              'signature did not match exactly once'}
DISTRIBUTION = {'steam': 'steam', 'ea app': 'ea', 'ea_app': 'ea', 'ea-app': 'ea'}
RESULTS = ('resolved', 'deferred', 'unresolved')
PATTERN_METHODS = ('hint', 'scan')
# Within one run a `[bind]` line outranks a legacy core line, which outranks the EA diagnostic report.
RANK = {'bind': 2, 'core': 1, 'ea': 0}


def hexnum(s):
    if s is None or s in ('', '-'):
        return None
    try:
        return int(s, 16)
    except ValueError:
        return None


def rva_str(v):
    return None if v is None else f'0x{v:x}'


def norm_bytes(s):
    s = (s or '').strip()
    if not s:
        return ''
    compact = re.sub(r'[^0-9A-Fa-f?]', '', s)
    if ' ' not in s and len(compact) % 2 == 0:
        return ' '.join(compact[i:i + 2] for i in range(0, len(compact), 2)).upper()
    return ' '.join(s.split()).upper()


def build_of_set(name):
    m = re.fullmatch(r'bindings(?:\.([A-Za-z0-9_-]+))?\.json', name or '')
    if not m:
        return 'unknown'
    return m.group(1) or 'steam'


def norm_build(b):
    b = (b or '').strip().lower()
    return DISTRIBUTION.get(b, b)


def ea_pass(when):
    return 'init' if when == 'core-init' else when


class Run:
    def __init__(self, source):
        self.source = source
        self.start = None
        self.pid = None
        self.lines = 0
        self.pe_timestamp = None
        self.exe_size = None
        self.set_file = None
        self.set_matched = None
        self.set_sha16 = None
        self.same_build = None
        self.set_total = None
        self.bind_builds = []
        self.summaries = []
        self.records = []
        self.formats = set()
        self.ea_no_anchor = set()
        self.truncated = False
        self.digest = hashlib.sha256()

    @property
    def key(self):
        return (self.start, self.pid) if self.start and self.pid is not None else ('sha', self.digest.hexdigest())

    @property
    def build(self):
        named = [b for b in self.bind_builds if b != 'unknown']
        if named:
            return named[-1]
        if self.set_file and self.set_matched:
            return build_of_set(self.set_file)
        return 'unknown'

    def identity(self):
        ident = {}
        if self.pe_timestamp is not None:
            ident['pe_timestamp'] = self.pe_timestamp
        if self.exe_size is not None:
            ident['size'] = self.exe_size
        if self.set_sha16 and self.same_build:
            ident['sha256_prefix'] = self.set_sha16
        return ident

    def total(self):
        for s in reversed(self.summaries):
            if s.get('total') is not None:
                return s['total']
        return self.set_total


def parse_bind_fields(body):
    out = {}
    for k, v in KV_RE.findall(body):
        if v.startswith('"'):
            v = re.sub(r'\\(.)', r'\1', v[1:-1])
        out[k] = v
    return out


def parse_lines(lines, source):
    runs, run = [], None
    core_pass, ea_when = 'init', 'init'
    for n, raw in enumerate(lines, 1):
        line = raw.rstrip('\r\n')
        attach = ATTACH_RE.search(line)
        if attach or run is None:
            if not attach and not line.strip():
                continue
            run = Run(source)
            runs.append(run)
            core_pass, ea_when = 'init', 'init'
        if line.startswith(('[swofty: kept the last', '[swofty: left out')):
            run.truncated = True
            continue
        run.lines += 1
        run.digest.update(line.encode('utf-8', 'replace') + b'\n')
        ts = TS_RE.match(line)
        if ts and run.start is None:
            run.start = ts.group(1)
        if attach:
            run.pid = int(attach.group(1))
        if '[' not in line and 'exe:' not in line:
            continue
        m = BIND_RE.search(line)
        if m:
            f = parse_bind_fields(m.group(1))
            run.formats.add('bind')
            if 'summary' in m.group(1).split()[:1]:
                s = {'build': f.get('build'), 'pass': f.get('pass')}
                for k in ('resolved', 'deferred', 'unresolved', 'total'):
                    s[k] = int(f[k]) if f.get(k, '').isdigit() else None
                for k in ('pe_timestamp', 'size'):
                    if f.get(k, '').lstrip('-').isdigit():
                        setattr(run, 'exe_size' if k == 'size' else k, int(f[k]))
                sha = f.get('sha256') or f.get('sha')
                if sha:
                    run.set_sha16, run.same_build = sha[:16].lower(), True
                if f.get('build'):
                    run.bind_builds.append(norm_build(f['build']))
                run.summaries.append(s)
                continue
            if not f.get('id'):
                continue
            if f.get('build'):
                run.bind_builds.append(norm_build(f['build']))
            matches = f.get('matches')
            run.records.append({
                'id': f['id'], 'source': 'bind', 'line': n, 'pass': f.get('pass') or '-',
                'result': f.get('result') if f.get('result') in RESULTS else 'unresolved',
                'method': f.get('method') or '-',
                'matches': int(matches) if matches and matches.isdigit() else None,
                'rva': hexnum(f.get('rva')), 'match': hexnum(f.get('match')),
                'site': hexnum(f.get('site')), 'verified': f.get('verified') == 'yes',
                'reason': f.get('reason') or '', 'site_bytes': norm_bytes(f.get('site_bytes')),
                'target_only': False})
            continue
        m = SELECT_RE.search(line)
        if m:
            if m.group(1):
                run.set_file, run.set_matched, run.pe_timestamp = m.group(1), True, int(m.group(2))
            else:
                run.set_file, run.set_matched = m.group(4) or m.group(5), False
                run.pe_timestamp = int(m.group(3))
            continue
        m = SET_RE.search(line)
        if m:
            run.set_total, run.set_sha16 = int(m.group(2)), m.group(3).lower()
            run.same_build = m.group(4) == 'this exe'
            continue
        m = EXE_RE.search(line)
        if m:
            run.exe_size = int(m.group(1))
            if run.pe_timestamp is None:
                run.pe_timestamp = int(m.group(2))
            continue
        m = STAGE_RE.search(line)
        if m:
            core_pass = m.group(1)
            continue
        m = CORE_BIND_RE.search(line)
        if m:
            run.formats.add('core')
            run.records.append({
                'id': m.group(1), 'source': 'core', 'line': n, 'pass': core_pass, 'result': 'resolved',
                'method': LEGACY_METHOD.get(m.group(6), m.group(6)), 'matches': None, 'rva': int(m.group(3), 16),
                'match': None, 'site': None, 'verified': False, 'reason': '', 'site_bytes': '',
                'target_only': True, 'kind': m.group(4), 'status': m.group(5)})
            continue
        m = CORE_WHY_RE.search(line)
        if m:
            why = m.group(2)
            if why.startswith('unresolved (') and why.endswith(')'):
                why = why[len('unresolved ('):-1]
            elif why not in LEGACY_WHY:
                continue
            run.formats.add('core')
            run.records.append({
                'id': m.group(1), 'source': 'core', 'line': n, 'pass': core_pass, 'result': 'unresolved',
                'method': '-', 'matches': None, 'rva': None, 'match': None, 'site': None, 'verified': False,
                'reason': why, 'site_bytes': '', 'target_only': False})
            continue
        m = CORE_MISSING_RE.search(line)
        if m:
            run.formats.add('core')
            run.records.append({
                'id': m.group(1), 'source': 'core', 'line': n, 'pass': core_pass, 'result': 'unresolved',
                'method': '-', 'matches': None, 'rva': None, 'match': None, 'site': None, 'verified': False,
                'reason': f'not in {m.group(2)}', 'site_bytes': '', 'target_only': False})
            continue
        m = EA_REPORT_RE.search(line)
        if m:
            ea_when = ea_pass(m.group(1))
            continue
        m = EA_BIND_RE.search(line)
        if m:
            bid, head, verdict, tail = m.groups()
            run.formats.add('ea')
            if verdict == 'no-anchor':
                run.ea_no_anchor.add(bid)
                continue
            rec = {'id': bid, 'source': 'ea', 'line': n, 'pass': ea_when, 'method': 'anchor', 'matches': None,
                   'rva': None, 'match': None, 'site': None, 'verified': False, 'reason': '', 'site_bytes': '',
                   'target_only': False, 'verdict': verdict}
            if verdict == 'resolved':
                s = EA_SITE_RE.search(head)
                rec.update(result='resolved', rva=int(s.group(1), 16) if s else None, target_only=True)
                if 'WARNING' in tail:
                    rec['reason'] = 'code read encrypted when resolved'
            else:
                rec.update(result='unresolved', method='-', reason=verdict)
                f = EA_FAIL_RE.match(tail)
                if f:
                    rec['reason'] = f'{verdict}: {f.group(1)}'
                    rec['site'] = int(f.group(2), 16)
                    rec['site_bytes'] = '' if f.group(3) == 'unreadable' else norm_bytes(f.group(3))
                elif tail.strip().startswith('('):
                    rec['reason'] = f"{verdict}: {tail.strip()[1:].rstrip(')')}"
            run.records.append(rec)
    return [r for r in runs if r.records or r.summaries or r.set_file]


def read_zip(path):
    out = []
    with zipfile.ZipFile(path) as z:
        for name in z.namelist():
            base = name.rsplit('/', 1)[-1]
            if base != 'loader.log':
                continue
            text = z.read(name).decode('utf-8', 'replace')
            out.append((f'{path}!{name}', text))
    return out


def read_dir(path):
    found = [p for p in (os.path.join(path, 'loader.log'), os.path.join(path, 'game-logs', 'loader.log'))
             if os.path.isfile(p)]
    if not found:
        # A sessions root or an unpacked Send Logs bundle: every loader.log a few levels down, as in the zip.
        for root, dirs, files in os.walk(path):
            if os.path.relpath(root, path).count(os.sep) >= 3:
                dirs[:] = []
            if 'loader.log' in files:
                found.append(os.path.join(root, 'loader.log'))
    out = []
    for p in found:
        with open(p, encoding='utf-8', errors='replace') as f:
            out.append((p, f.read()))
    return out


def load_inputs(paths):
    texts = []
    for p in paths:
        if os.path.isdir(p):
            got = read_dir(p)
        elif zipfile.is_zipfile(p):
            got = read_zip(p)
        elif os.path.isfile(p):
            with open(p, encoding='utf-8', errors='replace') as f:
                got = [(p, f.read())]
        else:
            raise SystemExit(f'capture: {p}: no such file or directory')
        if not got:
            raise SystemExit(f'capture: {p}: no loader.log found')
        texts.extend(got)
    runs = {}
    for source, text in texts:
        for run in parse_lines(text.splitlines(), source):
            prev = runs.get(run.key)
            # The session copy and game-logs copy are the same run; the longer one holds every line of the other.
            if prev is None or run.lines > prev.lines:
                runs[run.key] = run
    return sorted(runs.values(), key=lambda r: (r.start or '', r.source))


def read_subsystems(db=DB):
    out = {}
    for table in ('functions', 'globals'):
        p = os.path.join(db, f'{table}.tsv')
        if not os.path.isfile(p):
            continue
        with open(p, encoding='utf-8') as f:
            head = f.readline().rstrip('\n').split('\t')
            if 'name' not in head or 'subsystem' not in head:
                continue
            ni, si = head.index('name'), head.index('subsystem')
            for line in f:
                c = line.rstrip('\n').split('\t')
                if len(c) > max(ni, si) and c[si] and c[si] != '-':
                    out.setdefault(c[ni], c[si])
    return out


def pick(records):
    return max(records, key=lambda r: (RANK[r['source']], r['line']))


def analyse(runs, build=None, subsystems=None):
    subsystems = subsystems or {}
    groups = {}
    for run in runs:
        name = run.build
        if name == 'unknown' and build:
            name = build
        if build and name != build:
            continue
        groups.setdefault(name, []).append(run)
    out = {}
    for name, rs in groups.items():
        idents = {r.identity().get('pe_timestamp') for r in rs} - {None}
        # Runs of one build name on different exes (a patch) are kept apart: their hints are not interchangeable.
        if len(idents) > 1:
            for pe in sorted(idents):
                mine = [r for r in rs if r.identity().get('pe_timestamp') in (pe, None)]
                out[f'{name}@{pe}'] = summarise(name, mine, subsystems)
        else:
            out[name] = summarise(name, rs, subsystems)
    return out


def is_hint(rec):
    return rec['source'] == 'bind' and rec['result'] == 'resolved' and rec['verified'] \
        and rec['match'] is not None and rec['method'] in PATTERN_METHODS


def summarise(name, runs, subsystems):
    final, diag = {}, {}
    for run in runs:
        per = {}
        for rec in run.records:
            per.setdefault(rec['id'], []).append(rec)
        for bid, recs in per.items():
            best = dict(pick(recs), run=run.source)
            if best['result'] == 'unresolved' and not best['site_bytes']:
                donor = [r for r in recs if r['source'] == 'ea' and r['site_bytes']]
                if donor:
                    best['site_bytes'], best['site'] = donor[-1]['site_bytes'], donor[-1]['site']
            final[bid] = best
            failed = [r for r in recs if r['source'] == 'ea' and r['result'] == 'unresolved']
            if failed and best['source'] != 'ea' and best['result'] == 'resolved':
                diag[bid] = failed[-1]
            else:
                diag.pop(bid, None)
    latest = runs[-1]
    counts = {'resolved': 0, 'deferred': 0, 'unresolved': 0}
    for rec in final.values():
        counts[rec['result']] += 1
    total = next((r.total() for r in reversed(runs) if r.total() is not None), None)
    seen = sum(counts.values())
    cov = dict(counts, total=total, unreported=(total - seen) if total is not None and total >= seen else None)
    worklist = []
    for bid, rec in final.items():
        if rec['result'] != 'unresolved':
            continue
        site = next((rec[k] for k in ('site', 'match', 'rva') if rec[k] is not None), None)
        worklist.append({'id': bid, 'subsystem': subsystems.get(bid), 'pass': rec['pass'], 'method': rec['method'],
                         'reason': rec['reason'], 'site': rva_str(site), 'site_bytes': rec['site_bytes'],
                         'source': rec['source']})
    worklist.sort(key=lambda w: (w['subsystem'] is None, w['subsystem'] or '', w['id']))
    resolved = {bid: rec for bid, rec in sorted(final.items()) if rec['result'] == 'resolved'}
    hints = {bid: rec for bid, rec in resolved.items() if is_hint(rec)}
    target_only = {bid: rva_str(rec['rva']) for bid, rec in resolved.items() if rec['target_only']}
    unverified = {bid: {'rva': rva_str(rec['rva']), 'match': rva_str(rec['match']), 'method': rec['method'],
                        'verified': rec['verified']}
                  for bid, rec in resolved.items() if bid not in hints and not rec['target_only']}
    diagnostics = [{'id': bid, 'subsystem': subsystems.get(bid), 'pass': r['pass'], 'reason': r['reason'],
                    'site': rva_str(r['site']), 'site_bytes': r['site_bytes'], 'core_rva': rva_str(final[bid]['rva'])}
                   for bid, r in sorted(diag.items())]
    identity, set_file = {}, None
    for run in runs:
        identity.update(run.identity())
        set_file = run.set_file or set_file
    return {
        'build': name,
        'identity': identity,
        'set': set_file,
        'runs': [run_info(r) for r in runs],
        'coverage': cov,
        'summaries': [s for r in runs for s in r.summaries],
        'worklist': worklist,
        'hints': {bid: {'match': rva_str(rec['match'])} for bid, rec in hints.items()},
        'hint_sources': {bid: {'pass': rec['pass'], 'method': rec['method']} for bid, rec in hints.items()},
        'target_only': target_only,
        'unverified': unverified,
        'diagnostics': diagnostics,
        'ea_no_anchor': len(latest.ea_no_anchor - set(final)),
        'bindings': {bid: {k: (rva_str(v) if k in ('rva', 'match', 'site') else v) for k, v in rec.items()
                           if k != 'line'} for bid, rec in sorted(final.items())},
    }


def run_info(r):
    return {'source': r.source, 'start': r.start, 'pid': r.pid, 'build': r.build, 'set': r.set_file,
            'set_selected_by_pe_timestamp': r.set_matched, 'set_sha16': r.set_sha16, 'same_build': r.same_build,
            'identity': r.identity(), 'formats': sorted(r.formats), 'truncated': r.truncated, 'lines': r.lines}


def check_identity(log_ident, block):
    compared, bad = [], []
    if 'pe_timestamp' in log_ident and block.get('pe_timestamp') not in (None, 0, ''):
        compared.append('pe_timestamp')
        if int(block['pe_timestamp']) != log_ident['pe_timestamp']:
            bad.append(f"pe_timestamp {log_ident['pe_timestamp']} != {block['pe_timestamp']}")
    if 'sha256_prefix' in log_ident and block.get('sha256'):
        compared.append('sha256')
        if not str(block['sha256']).lower().startswith(log_ident['sha256_prefix']):
            bad.append(f"sha256 {log_ident['sha256_prefix']}... != {block['sha256']}")
    if 'size' in log_ident and block.get('size') not in (None, 0, ''):
        compared.append('size')
        if int(block['size']) != log_ident['size']:
            bad.append(f"size {log_ident['size']} != {block['size']}")
    if bad:
        return 'the log is from a different exe than the overlay: ' + '; '.join(bad)
    if not compared:
        return 'the log carries no build identity (pe_timestamp, sha256 or size) to check against the overlay'
    return None


def json_indent(text):
    m = re.search(r'\n( +)"', text)
    return len(m.group(1)) if m else 1


def write_hints(path, summary):
    if not os.path.isfile(path):
        raise SystemExit(f'capture: {path} does not exist; the overlay (with its build block) must exist first')
    with open(path, encoding='utf-8') as f:
        before = f.read()
    doc = json.loads(before)
    err = check_identity(summary['identity'], doc.get('build') or {})
    if err:
        raise SystemExit(f'capture: refusing to write {path}: {err}')
    hints = doc.get('hints')
    created = hints is None
    if created:
        hints = doc['hints'] = {}
    added, changed = [], []
    for bid, h in summary['hints'].items():
        old = hints.get(bid)
        if old is None:
            hints[bid] = dict(h)
            added.append(bid)
        elif old.get('match') != h['match']:
            changed.append((bid, old.get('match')))
            old['match'] = h['match']
    diff = ''
    if created or added or changed:
        doc['hints'] = dict(sorted(hints.items()))
        after = json.dumps(doc, indent=json_indent(before), ensure_ascii=False) + '\n'
        inside = os.path.abspath(path).startswith(REPO + os.sep)
        rel = os.path.relpath(path, REPO) if inside else path
        labels = (f'a/{rel}', f'b/{rel}') if inside else (rel, rel)
        diff = ''.join(difflib.unified_diff(before.splitlines(True), after.splitlines(True), *labels))
        with open(path, 'w', encoding='utf-8') as f:
            f.write(after)
    return {'path': path, 'added': added, 'changed': changed, 'unchanged': len(summary['hints']) - len(added) -
            len(changed), 'diff': diff}


def fmt_cov(c):
    parts = [f"resolved {c['resolved']}", f"deferred {c['deferred']}", f"unresolved {c['unresolved']}"]
    if c['unreported'] is not None:
        parts.append(f"unreported {c['unreported']}")
    parts.append(f"total {c['total'] if c['total'] is not None else '?'}")
    return ', '.join(parts)


def table(rows, head):
    widths = [max(len(str(x)) for x in col) for col in zip(head, *rows)]
    return ['  ' + '  '.join(str(x).ljust(w) for x, w in zip(r, widths)).rstrip() for r in [head] + rows]


def human(result):
    out = []
    for key, s in result['builds'].items():
        ident = ', '.join(f'{k} {v}' for k, v in s['identity'].items()) or 'no identity in the log'
        out.append(f"== build {key} ({ident}; set {s['set'] or '?'}) ==")
        for r in s['runs']:
            note = ' [truncated]' if r['truncated'] else ''
            out.append(f"  run {r['start'] or '?'} pid {r['pid']} from {r['source']} "
                       f"(lines: {', '.join(r['formats']) or 'none'}){note}")
        out.append(f"coverage: {fmt_cov(s['coverage'])}")
        legacy = all('bind' not in r['formats'] for r in s['runs'])
        if legacy:
            out.append('  older log format: only bindings the core was asked for are logged; '
                       'unreported ones were not attempted')
        if s['ea_no_anchor']:
            out.append(f"  EA report: {s['ea_no_anchor']} binding(s) with no string anchor were not evaluated")
        for sm in s['summaries'][-3:]:
            out.append(f"  core summary pass {sm['pass']}: resolved {sm['resolved']}, deferred {sm['deferred']}, "
                       f"unresolved {sm['unresolved']}, total {sm['total']}")
        out.append(f"override worklist ({len(s['worklist'])}):")
        if s['worklist']:
            rows = [[w['subsystem'] or '?', w['id'], w['pass'], w['reason'] or '-', w['site'] or '-',
                     w['site_bytes'] or '-'] for w in s['worklist']]
            out += table(rows, ['subsystem', 'id', 'pass', 'reason', 'site', 'site_bytes'])
        out.append(f"hints ({len(s['hints'])} verified):")
        if s['hints']:
            rows = [[bid, h['match'], s['hint_sources'][bid]['method'], s['hint_sources'][bid]['pass']]
                    for bid, h in s['hints'].items()]
            out += table(rows, ['id', 'match', 'method', 'pass'])
        if s['target_only']:
            out.append(f"target-only, not hints ({len(s['target_only'])}): the log names the resolved target, "
                       'not the pattern match')
            out += table([[bid, v] for bid, v in s['target_only'].items()], ['id', 'target'])
        if s['unverified']:
            out.append(f"resolved without a verified pattern match, not hints ({len(s['unverified'])}):")
            out += table([[bid, u['rva'] or '-', u['match'] or '-', u['method'], 'yes' if u['verified'] else 'no']
                          for bid, u in s['unverified'].items()], ['id', 'target', 'match', 'method', 'verified'])
        if s['diagnostics']:
            out.append(f"anchor diagnostics that failed where the core resolved ({len(s['diagnostics'])}):")
            rows = [[d['id'], d['core_rva'] or '-', d['reason'], d['site'] or '-', d['site_bytes'] or '-']
                    for d in s['diagnostics']]
            out += table(rows, ['id', 'core target', 'anchor verdict', 'site', 'site_bytes'])
        out.append('')
    return '\n'.join(out)


def main(argv=None):
    ap = argparse.ArgumentParser(description='Binding coverage, override worklist and hints from loader logs.')
    ap.add_argument('inputs', nargs='+', help='loader.log, a launcher session dir, or a Send Logs zip (or its parts)')
    ap.add_argument('--build', help='only this build (steam, ea, ...); also names runs whose build the log omits')
    ap.add_argument('--json', action='store_true', help='machine-readable output')
    ap.add_argument('--write', action='store_true', help='merge verified hints into re/db/builds/<build>.json')
    ap.add_argument('--builds-dir', default=os.path.join(DB, 'builds'), help=argparse.SUPPRESS)
    ap.add_argument('--db', default=DB, help=argparse.SUPPRESS)
    a = ap.parse_args(argv)
    runs = load_inputs(a.inputs)
    if not runs:
        raise SystemExit('capture: no loader output in the input')
    builds = analyse(runs, a.build, read_subsystems(a.db))
    if not builds:
        raise SystemExit(f'capture: no run for build {a.build}; found ' +
                         ', '.join(sorted({r.build for r in runs})))
    result = {'inputs': a.inputs, 'builds': builds}
    if a.write:
        if len(builds) != 1:
            raise SystemExit('capture: --write needs one build; pass --build (found ' + ', '.join(builds) + ')')
        (key, summary), = builds.items()
        if summary['build'] == 'unknown' or '@' in key:
            raise SystemExit(f'capture: --write needs a known build with one exe identity (got {key}); pass --build')
        result['write'] = write_hints(os.path.join(a.builds_dir, f"{summary['build']}.json"), summary)
    if a.json:
        json.dump(result, sys.stdout, indent=1)
        sys.stdout.write('\n')
    else:
        sys.stdout.write(human(result))
        w = result.get('write')
        if w:
            sys.stdout.write(f"\nwrote {w['path']}: {len(w['added'])} added, {len(w['changed'])} changed, "
                             f"{w['unchanged']} unchanged\n" if w['diff'] else
                             f"\n{w['path']}: hints already up to date\n")
            sys.stdout.write(w['diff'])
    return 0


if __name__ == '__main__':
    sys.exit(main())

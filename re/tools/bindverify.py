#!/usr/bin/env python3
"""Resolve every binding of an emitted set against a static image of its build, with the core resolver's rules.

  bindverify.py --build ea --image F1_25.<ts>.mem          EA set against a decrypted flat image dump
  bindverify.py --build steam --image F1_25.exe            Steam set against the exe file
  bindverify.py --set native/dist/.../bindings.ea.json --image ...
                                                           check an emitted file instead of generating the set
  --exe-size N    size of the exe on disk (a dump does not record it); the core uses hints only when it matches
  --scan          ignore match hints, as SWOFTY_RESOLVE=scan does
  --json          print the per-binding results as JSON
  --compare EXE   also flag bindings whose match surroundings or callee differ from the Steam exe's

The image is a PE file or a flat dump whose file offset is the RVA (a file starting with MZ whose length is its
SizeOfImage). Both are read with the PE header found in the image itself, as the core reads its own module.
The order mirrors resolve_binding in native/src/core/manager.cpp: the byte-checked match hint, then the section
scan (exactly one hit), then the string anchor; apply_match checks the resolve step, expect and the target section.
A packed set also applies attempt(): pages that read encrypted defer the binding.
Exit status 0 only when every binding that is not marked unresolvable resolves.
"""
import argparse, json, math, mmap, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

SCN_EXECUTE, SCN_READ, SCN_WRITE = 0x20000000, 0x40000000, 0x80000000
ENCRYPTED, PLAIN = 7.5, 6.5
CODE_KINDS = ('func', 'site', 'call')


class Section:
    def __init__(self, name, va, vs, rs, ro, flags):
        self.name, self.va, self.vs, self.rs, self.ro, self.flags = name, va, vs, rs, ro, flags
        self.size = vs or rs

    def has(self, rva):
        return 0 <= rva - self.va < self.size


def parse_headers(buf):
    if buf[:2] != b'MZ':
        raise ValueError('not a PE image (no MZ)')
    pe = struct.unpack_from('<I', buf, 0x3c)[0]
    if buf[pe:pe + 4] != b'PE\0\0':
        raise ValueError('not a PE image (no PE signature)')
    nsec, stamp = struct.unpack_from('<HI', buf, pe + 6)
    optsz = struct.unpack_from('<H', buf, pe + 20)[0]
    opt = pe + 24
    if struct.unpack_from('<H', buf, opt)[0] != 0x20b:
        raise ValueError('not a PE32+ image')
    image_base, salign = struct.unpack_from('<QI', buf, opt + 24)
    size_image, size_headers = struct.unpack_from('<II', buf, opt + 56)
    exc = struct.unpack_from('<II', buf, opt + 112 + 8 * 3)
    secs = []
    for i in range(nsec):
        o = opt + optsz + 40 * i
        name, vs, va, rs, ro = struct.unpack_from('<8sIIII', buf, o)
        flags = struct.unpack_from('<I', buf, o + 36)[0]
        secs.append(Section(name.split(b'\0', 1)[0].decode('latin-1'), va, vs, rs, ro, flags))
    return {'stamp': stamp, 'image_base': image_base, 'salign': salign, 'size_image': size_image,
            'size_headers': size_headers, 'exception': exc, 'sections': secs}


class Image:
    """The module as the running game maps it; offset = RVA. Everything inside SizeOfImage reads as committed."""

    def __init__(self, path):
        self.path = path
        f = open(path, 'rb')
        raw = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)
        h = parse_headers(raw)
        self.flat = len(raw) == h['size_image']
        if self.flat:
            self.mem = raw
        else:
            mem = bytearray(h['size_image'])
            mem[:h['size_headers']] = raw[:h['size_headers']]
            for s in h['sections']:
                n = min(s.rs, -(-s.size // h['salign']) * h['salign'], h['size_image'] - s.va)
                chunk = raw[s.ro:s.ro + n]
                mem[s.va:s.va + len(chunk)] = chunk
            self.mem = mem
            raw.close()
        f.close()
        h = parse_headers(self.mem)
        self.stamp, self.image_base, self.size = h['stamp'], h['image_base'], h['size_image']
        self.sections = h['sections']
        self.exception = h['exception']
        self._pdata = None
        self._strings = {}
        self._funcs = {}
        self._scans = {}

    def readable(self, rva, n):
        return 0 <= rva and rva + n <= self.size

    def read(self, rva, n):
        return bytes(self.mem[rva:rva + n]) if self.readable(rva, n) else None

    def i32(self, rva):
        return struct.unpack_from('<i', self.mem, rva)[0]

    def section_of(self, rva):
        return next((s for s in self.sections if s.has(rva)), None)

    def section_by_name(self, name):
        return next((s for s in self.sections if len(name) <= 8 and s.name == name), None)

    def section_named(self, rva, name):
        s = self.section_of(rva)
        return s is not None and len(name) <= 8 and s.name == name

    def is_code(self, rva):
        s = self.section_of(rva)
        return s is not None and bool(s.flags & SCN_EXECUTE) and not s.flags & SCN_WRITE

    def in_exec(self, rva):
        s = self.section_of(rva)
        return s is not None and bool(s.flags & SCN_EXECUTE)

    def page_entropy(self, rva):
        p = rva & ~0xfff
        b = self.read(p, 0x1000)
        if b is None:
            return -1.0
        cnt = [0] * 256
        for x in b:
            cnt[x] += 1
        return -sum(c / 4096 * math.log2(c / 4096) for c in cnt if c)

    def section_entropy(self, s):
        size, total, ok = s.size, 0.0, 0
        for k in range(16):
            off = (int(k / 16 * (size - 0x1000)) & ~0xfff) if size > 16 * 0x1000 else k * 0x1000
            if k and off + 0x1000 > size:
                break
            e = self.page_entropy(s.va + off)
            if e >= 0:
                ok, total = ok + 1, total + e
        return total / ok if ok else -1.0

    def page_encrypted(self, rva):
        e = self.page_entropy(rva)
        return e < 0 or e > ENCRYPTED

    def matches_at(self, rva, pat):
        if not pat or not self.readable(rva, len(pat)):
            return False
        b = self.mem[rva:rva + len(pat)]
        return all(p is None or b[k] == p for k, p in enumerate(pat))

    def scan(self, section, text):
        """version.c scan(): every section of that name, every (overlapping) hit counted."""
        key = (section, text)
        if key in self._scans:
            return self._scans[key]
        pat = loader_tokens(text)
        hits, first = 0, None
        if pat and any(p is not None for p in pat):
            lit, off = longest_literal(pat)
            for s in self.sections:
                if s.name != section:
                    continue
                lo, hi = s.va, min(s.va + s.size, self.size)
                pos = lo + off
                while True:
                    i = self.mem.find(lit, pos, hi - (len(pat) - off - len(lit)))
                    if i < 0:
                        break
                    start = i - off
                    if start >= lo and start + len(pat) <= hi and self.matches_at(start, pat):
                        if not hits:
                            first = start
                        hits += 1
                    pos = i + 1
        self._scans[key] = (hits, first if hits == 1 else None, first)
        return self._scans[key]

    def locate_string(self, text):
        """locate_string(): NUL-terminated, in readable non-executable sections, exactly once."""
        if text in self._strings:
            return self._strings[text]
        want = text.encode('latin-1') + b'\0'
        hits, hit = 0, None
        for s in self.sections:
            if hits >= 2:
                break
            if not s.flags & SCN_READ or s.flags & SCN_EXECUTE:
                continue
            lo, hi = s.va, min(s.va + s.size, self.size)
            pos = lo
            while True:
                i = self.mem.find(want, pos, hi)
                if i < 0:
                    break
                if not hits:
                    hit = i
                hits += 1
                pos = i + 1
        self._strings[text] = hit if hits == 1 else None
        return self._strings[text]

    def pdata(self):
        if self._pdata is None:
            va, size = self.exception
            starts, ends = [], []
            if va and size >= 12 and self.readable(va, size - size % 12):
                for b, e, _ in struct.iter_unpack('<III', self.mem[va:va + size - size % 12]):
                    starts.append(b)
                    ends.append(e)
            self._pdata = (starts, ends)
        return self._pdata

    def func_base_of(self, rva):
        """func_base_of(): binary search over the exception directory as the image holds it (assumed sorted)."""
        starts, ends = self.pdata()
        lo, hi = 0, len(starts)
        while lo < hi:
            mid = (lo + hi) // 2
            if starts[mid] <= rva:
                lo = mid + 1
            else:
                hi = mid
        if lo and starts[lo - 1] <= rva < ends[lo - 1]:
            return starts[lo - 1]
        return None

    def rel32_refs(self, target):
        """Every q in read-only code with q + 4 + int32(q) == target. The top 16 bits of the wanted displacement stay
        fixed across a run of up to 65536 consecutive q, so each run is one find of those two bytes."""
        out = []
        for s in self.sections:
            if not s.flags & SCN_EXECUTE or not s.flags & SCN_READ or s.flags & SCN_WRITE:
                continue
            q, last = s.va, min(s.va + s.size, self.size) - 4
            while q <= last:
                d = (target - 4 - q) & 0xffffffff
                end = min(last, q + (d & 0xffff))
                hi = struct.pack('<H', d >> 16)
                pos = q + 2
                while True:
                    i = self.mem.find(hi, pos, end + 4)
                    if i < 0:
                        break
                    r = i - 2
                    if (target - 4 - r) & 0xffff == struct.unpack_from('<H', self.mem, r)[0]:
                        out.append(r)
                    pos = i + 1
                q = end + 1
        return out

    def anchor_func(self, text):
        """anchor_func(): every rel32 reference to the string must sit in one function of the exception directory."""
        if text in self._funcs:
            return self._funcs[text]
        s = self.locate_string(text)
        fbase, conflict, sites = None, False, 0
        if s is not None:
            for q in self.rel32_refs(s):
                f = self.func_base_of(q)
                if f is None:
                    continue
                sites += 1
                if fbase is None:
                    fbase = f
                elif f != fbase:
                    conflict = True
        self._funcs[text] = None if conflict or not sites else fbase
        return self._funcs[text]


def loader_tokens(text):
    """parse_pattern() in version.c and scancache::tokens(): '?' starts a wildcard, else one or two hex digits."""
    out = []
    for t in text.split():
        if t[0] == '?':
            out.append(None)
            continue
        try:
            v = int(t, 16)
        except ValueError:
            return []
        if len(t) > 2 or not 0 <= v <= 255:
            return []
        out.append(v)
    return out


def longest_literal(pat):
    best, cur = (0, 0), None
    for k, p in enumerate(pat + [None]):
        if p is not None:
            cur = k if cur is None else cur
        elif cur is not None:
            if k - cur > best[1] - best[0]:
                best = (cur, k)
            cur = None
    return bytes(pat[best[0]:best[1]]), best[0]


def hex_bytes(text):
    out = []
    for t in (text or '').split():
        try:
            out.append(int(t, 16) & 0xff)
        except ValueError:
            break
    return out


def num(v, default):
    if v is None:
        return default
    if isinstance(v, bool):
        return default
    if isinstance(v, (int, float)):
        return int(v)
    try:
        return int(str(v), 0)
    except ValueError:
        return 0


class Binding:
    def __init__(self, j):
        res = j.get('resolve') or {}
        self.id, self.kind, self.status = j['id'], j.get('kind', ''), j.get('status', '')
        self.section, self.pattern = j.get('section') or '', j.get('pattern') or ''
        self.rkind = res.get('kind', 'addr') if isinstance(res.get('kind'), str) else 'addr'
        self.expect = hex_bytes(j.get('expect'))
        self.anchor, self.insn, self.len = num(j.get('anchor'), 0), num(res.get('insn'), 0), num(res.get('len'), 0)
        self.rva, self.match = num(j.get('rva'), -1), num(j.get('match'), -1)
        sa = j.get('string_anchor')
        self.sa_string = sa.get('string', '') if isinstance(sa, dict) else ''
        self.sa_func_rva = num(sa.get('func_rva'), -1) if isinstance(sa, dict) else -1
        self.sa_func_off = num(sa.get('func_offset'), 0) if isinstance(sa, dict) else 0
        self.unresolvable = (j.get('reason') or 'the set gives no reason') if self.status == 'unresolvable' else ''


class Resolver:
    def __init__(self, img, doc, exe_size=None, kb_first=True):
        self.img = img
        b = doc['build']
        self.same_build = num(b.get('pe_timestamp'), 0) == img.stamp
        set_size = num(b.get('size'), -1)
        self.kb_first = kb_first and not (self.same_build and set_size > 0 and exe_size is not None and exe_size != set_size)
        self.packed = b.get('packed') if isinstance(b.get('packed'), bool) else None
        if self.packed is None:
            cs = next((s for s in img.sections if s.flags & SCN_EXECUTE and s.flags & SCN_READ and not s.flags & SCN_WRITE),
                      next((s for s in img.sections if s.flags & SCN_EXECUTE), None))
            self.packed = cs is not None and img.section_entropy(cs) > ENCRYPTED
        self.bindings = [Binding(j) for j in doc['bindings']]
        self._sections = {}

    def apply_match(self, b, match):
        img = self.img
        at, i = match + b.anchor, match + b.insn
        target = at
        if b.rkind in ('rel32', 'riprel') and not img.readable(min(at, i), abs(at - i) + 16):
            return None, 'anchor offset is not mapped on this build'
        if b.rkind == 'rel32':
            c0, c1 = img.mem[i], img.mem[i + 1]
            branch = c0 in (0xE8, 0xE9) and at == i + 1
            jcc = c0 == 0x0F and (c1 & 0xF0) == 0x80 and at == i + 2
            if not branch and not jcc:
                return None, 'anchor is not the rel32 of a call or jump'
            target = at + 4 + img.i32(at)
        elif b.rkind == 'riprel':
            if b.anchor <= b.insn or b.anchor + 4 > b.insn + b.len:
                return None, 'anchor is not inside the RIP-relative instruction'
            target = i + b.len + img.i32(at)
        elif b.rkind != 'addr':
            return None, 'unknown resolve kind'
        if b.expect and (not img.readable(at, len(b.expect)) or list(img.mem[at:at + len(b.expect)]) != b.expect):
            return None, 'expect bytes differ'
        code = b.kind in CODE_KINDS
        own = img.section_of(target)
        own_exec_site = (b.kind in ('site', 'call') and own is not None and bool(own.flags & SCN_EXECUTE)
                         and img.section_named(target, b.section))
        if (not img.is_code(target) and not own_exec_site) if code else own is None:
            return None, 'target is not in read-only code' if code else 'target is outside the exe'
        if self.same_build and b.rva >= 0 and target != b.rva:
            return None, 'target differs from the KB build'
        return target, None

    def by_pattern(self, b, tr):
        img = self.img
        pat = loader_tokens(b.pattern)
        if self.kb_first and self.same_build and b.match >= 0:
            tr['match'] = tr['site'] = b.match
            if img.section_named(b.match, b.section) and img.matches_at(b.match, pat):
                tr['method'], tr['matches'] = 'hint', 1
                target, why = self.apply_match(b, b.match)
                tr['verified'] = target is not None
                return target, why
            tr['verified'] = False
            tr['hint_miss'] = True
        hits, match, first = img.scan(b.section, b.pattern)
        tr['matches'] = hits
        if first is not None and tr.get('site') is None:
            tr['near'] = first
        if match is None:
            return None, 'signature did not match exactly once'
        tr['method'], tr['match'], tr['site'] = 'scan', match, match
        target, why = self.apply_match(b, match)
        tr['verified'] = target is not None
        return target, why

    def by_anchor(self, b, tr):
        if not b.sa_string:
            return None, 'no string anchor'
        f = self.img.anchor_func(b.sa_string)
        if f is None:
            return None, 'string anchor did not locate a unique function'
        if tr.get('site') is None:
            tr['site'] = f + b.sa_func_off
        if self.same_build and b.sa_func_rva >= 0 and f != b.sa_func_rva:
            return None, 'string anchor function differs from the KB build'
        return self.apply_match(b, f + b.sa_func_off)

    def resolve_binding(self, b):
        tr = {'method': '-', 'matches': None, 'match': None, 'site': None, 'verified': None, 'target': None,
              'reason': ''}
        if b.unresolvable:
            tr['reason'] = 'unresolvable: ' + b.unresolvable
            return tr
        legacy = not b.pattern
        target, pw, aw = None, 'signature did not match exactly once', 'no string anchor'
        if legacy:
            pw = 'the set gives no pattern (legacy format)'
        elif self.img.section_by_name(b.section) is None:
            pw = f'section {b.section} is not in this exe'
        else:
            target, why = self.by_pattern(b, tr)
            if target is None:
                pw = why
        if target is None and b.sa_string:
            target, why = self.by_anchor(b, tr)
            if target is not None:
                tr['method'] = 'anchor' if legacy else 'anchor-fallback'
                tr['match'], tr['verified'] = None, None
            else:
                aw = why
        if target is None:
            tr['method'] = '-'
            tr['reason'] = pw if not b.sa_string else f'pattern: {pw}; string anchor: {aw}'
        tr['target'] = target
        return tr

    def section_now(self, name):
        if name not in self._sections:
            s = self.img.section_by_name(name)
            self._sections[name] = self.img.section_entropy(s) if s else -1.0
        return self._sections[name]

    def attempt(self, b):
        """attempt() at the decrypted stage: on a packed build, code that still reads encrypted defers."""
        s = self.img.section_by_name(b.section) if self.packed else None
        guarded = s is not None and bool(s.flags & SCN_EXECUTE)
        if not guarded or b.unresolvable:
            tr = self.resolve_binding(b)
            tr['result'] = 'resolved' if tr['target'] is not None else 'unresolvable' if b.unresolvable else 'unresolved'
            return tr
        e = self.section_now(b.section)
        if e < 0 or e > ENCRYPTED:
            return {'method': '-', 'result': 'deferred', 'target': None,
                    'reason': f'code section {b.section} reads encrypted (entropy {max(e, 0):.2f})'}
        tr = self.resolve_binding(b)
        if tr['target'] is not None:
            m = tr['match']
            bad = m if m is not None and self.img.page_encrypted(m) else \
                tr['target'] if self.img.in_exec(tr['target']) and self.img.page_encrypted(tr['target']) else None
            if bad is None:
                tr['result'] = 'resolved'
            else:
                tr['result'], tr['reason'] = 'deferred', f'the page at base+{bad:#x} still reads encrypted'
            return tr
        partly = e > PLAIN or (tr['site'] is not None and self.img.page_encrypted(tr['site']))
        tr['result'] = 'deferred' if partly else 'unresolved'
        return tr

    def run(self):
        out = []
        for b in self.bindings:
            tr = self.attempt(b)
            tr['id'] = b.id
            out.append(tr)
        return out


def coverage(results):
    c = {'hint': 0, 'scan': 0, 'anchor': 0, 'unresolved': 0, 'deferred': 0, 'unresolvable': 0}
    for r in results:
        if r['result'] == 'resolved':
            c['anchor' if r['method'].startswith('anchor') else r['method']] += 1
        else:
            c[r['result']] += 1
    c['total'] = len(results)
    return c


def follow_jumps(img, rva, hops=4):
    while hops and img.readable(rva, 5) and img.mem[rva] == 0xE9:
        rva, hops = rva + 5 + img.i32(rva + 1), hops - 1
    return rva


def similarity(a, b):
    return sum(x == y for x, y in zip(a, b)) / max(min(len(a), len(b)), 1) if a and b else 0.0


def compare(ref_img, ref_doc, ref_results, img, doc, results, context=0.4, callee=0.5):
    """Flag bindings that resolve on both builds but probably not to the same place. A pattern made of protector
    filler can match once elsewhere, so the bytes around the match (outside the pattern) and, for a call, the
    callee's first bytes must look like the reference build's."""
    ref = {b['id']: (b, r) for b, r in zip(ref_doc['bindings'], ref_results)}
    out = []
    for b, r in zip(doc['bindings'], results):
        if b['id'] not in ref or r['target'] is None:
            continue
        rb, rr = ref[b['id']]
        if rr['target'] is None:
            continue
        m, rm = r['match'] if r['match'] is not None else r['target'], rr['match'] if rr['match'] is not None else rr['target']
        n, rn = len(b['pattern'].split()), len(rb['pattern'].split())
        around = max(similarity(img.read(m - 16, 16), ref_img.read(rm - 16, 16)),
                     similarity(img.read(m + n, 16), ref_img.read(rm + rn, 16)))
        why = []
        if around < context:
            why.append(f'bytes around the match look {around:.2f} alike')
        t, rt = r['target'], rr['target']
        if ref_img.readable(rt, 5) and ref_img.mem[rt] == 0xE8:
            if not (img.readable(t, 5) and img.mem[t] == 0xE8):
                why.append('the reference site is a call and this one is not')
            else:
                c = follow_jumps(img, t + 5 + img.i32(t + 1))
                rc = follow_jumps(ref_img, rt + 5 + ref_img.i32(rt + 1))
                sim = similarity(img.read(c, 16), ref_img.read(rc, 16))
                if sim < callee:
                    why.append(f'the callee {c:#x} looks {sim:.2f} like the reference callee {rc:#x}')
        if why:
            out.append((b['id'], '; '.join(why)))
    return out


def gen_set(build):
    import bindgen
    kb_build, bindings, _, _ = bindgen.load()
    kb_name, overlays = bindgen.load_overlays(kb_build)
    if build not in overlays:
        raise SystemExit(f'bindverify: no overlay re/db/builds/{build}.json')
    return bindgen.build_set(overlays[build], bindings, build == kb_name)


def pe_stamp(path):
    with open(path, 'rb') as f:
        return parse_headers(f.read(0x1000))['stamp']


def is_kb_build(path):
    import bindgen
    return pe_stamp(path) == bindgen.load()[0]['pe_timestamp']


def gen_set_for(img):
    import bindgen
    kb_build, bindings, _, _ = bindgen.load()
    kb_name, overlays = bindgen.load_overlays(kb_build)
    for name, overlay in overlays.items():
        if overlay['build']['pe_timestamp'] == img.stamp:
            return bindgen.build_set(overlay, bindings, name == kb_name)
    raise SystemExit(f'bindverify: no overlay in re/db/builds has PE timestamp {img.stamp}')


def verify(image, doc, exe_size=None):
    img = image if isinstance(image, Image) else Image(image)
    return Resolver(img, doc, exe_size).run()


def rx(v):
    return '-' if v is None else f'{v:#x}'


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--image', required=True)
    ap.add_argument('--build', help='overlay name in re/db/builds (generates the set from re/db)')
    ap.add_argument('--set', help='an emitted bindings JSON file')
    ap.add_argument('--exe-size', type=int)
    ap.add_argument('--scan', action='store_true', help='ignore match hints, as SWOFTY_RESOLVE=scan does')
    ap.add_argument('--json', action='store_true')
    ap.add_argument('--all', action='store_true', help='also list resolved bindings')
    ap.add_argument('--compare', metavar='STEAM_EXE', help='also check that each binding lands on the same code as on the KB '
                                                           'build (Steam set, hints used)')
    a = ap.parse_args(argv)
    if bool(a.build) == bool(a.set):
        ap.error('pass exactly one of --build and --set')
    doc = json.load(open(a.set)) if a.set else gen_set(a.build)
    img = Image(a.image)
    if a.exe_size is None and not img.flat:
        a.exe_size = os.path.getsize(a.image)
    res = Resolver(img, doc, a.exe_size, kb_first=not a.scan)
    results = res.run()
    cov = coverage(results)
    if a.json:
        print(json.dumps({'build': doc['build'], 'image': {'pe_timestamp': img.stamp, 'flat': img.flat},
                          'same_build': res.same_build, 'kb_first': res.kb_first, 'coverage': cov,
                          'bindings': [{k: (rx(v) if k in ('match', 'site', 'target', 'near') else v)
                                        for k, v in r.items()} for r in results]}, indent=1))
    else:
        print(f'bindverify: {doc["build"]["distribution"]} set {doc["build"]["sha256"][:16]} on {a.image}: '
              f'image PE timestamp {img.stamp}, {"same build" if res.same_build else "a different build"}, '
              f'hints {"used" if res.kb_first and res.same_build else "not used"}, '
              f'{"packed" if res.packed else "not packed"}')
        for r in results:
            if a.all or r['result'] != 'resolved':
                print(f'  {r["id"]:48s} {r["result"]:12s} {r["method"]:15s} matches={r.get("matches")} '
                      f'match={rx(r.get("match"))} rva={rx(r.get("target"))} site={rx(r.get("site") or r.get("near"))} '
                      f'{r["reason"]}')
        print('coverage: ' + ', '.join(f'{k} {v}' for k, v in cov.items()))
    bad = sum(r['result'] not in ('resolved', 'unresolvable') for r in results)
    if a.compare:
        ref_img, ref_doc = Image(a.compare), gen_set('steam')
        odd = compare(ref_img, ref_doc, Resolver(ref_img, ref_doc).run(), img, doc, results)
        for i, why in odd:
            print(f'  differs from steam: {i}: {why}')
        print(f'compare: {len(odd)} binding(s) look like different code than on the KB build')
        bad += len(odd)
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())

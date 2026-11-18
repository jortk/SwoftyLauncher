#!/usr/bin/env python3
"""Check the core's runtime::Sig declarations against two static images of the game (the KB build and a port).

  sigverify.py                              every Sig of the default sources, Steam exe against the EA dump
  sigverify.py --src a.cpp b.cpp            Sigs of these sources
  sigverify.py --only S_PUSH,S_XML          a subset
  sigverify.py --json out.json              also write the per-signature results
  sigverify.py --mk RVA [--kind riprel|rel32] [--back N] [--max N]
                                            print a pattern at the KB RVA that matches once on BOTH images
  sigverify.py --refs RVA [--on port] [--section NAME]
                                            code sites whose rip-relative operand or rel32 reaches RVA (the image's own section name)
  sigverify.py --dis ref|port RVA [LEN]     disassemble
  sigverify.py --twin RVA                   the port's address of the KB function at RVA, by a both-unique pattern
  sigverify.py --disp RVA LEN HEX[,HEX]     per image: instructions in [RVA, RVA+LEN) of the KB function and of
                                            its --twin that use one of these displacements

Images are a PE file or a flat dump (file offset = RVA), read as bindverify.py reads them. A Sig's section is the
KB build's name; the port's name comes from re/db/builds/<build>.json "sections" (.xtls -> .rdata on EA).
The rules are runtime::find's (native/src/core/stage.cpp): the pattern must match exactly once in the section;
addr resolves to match + anchor, rel32 and riprel to match + insn + len + int32 at match + anchor.

Per signature: hits on each image, the KB target against the declared rva, and whether both targets are the same
code. Function entries compare the first 0x60 bytes as instructions with rip displacements, branch and call
targets masked (ratio of equal instructions). Sites inside a function (mid hooks) must have identical
instructions for 0x20 bytes at the hook point. A protector thunk (a jump to a body relocated out of the code section)
compares the relocated bodies. A Sig that scans the protector's section itself is reported and is not a failure when
it is unique on the KB build: the core hooks there on the KB build only. Data targets (globals, vtables) compare the section and, for a
vtable, the first slots' functions. Exit status 0 only when every signature is unique on both and the same.
"""
import argparse, difflib, json, os, re, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import numpy as np
from capstone import Cs, CS_ARCH_X86, CS_MODE_64
from capstone.x86 import X86_OP_MEM, X86_REG_RIP, X86_GRP_JUMP, X86_GRP_CALL, X86_OP_IMM
from bindverify import Image, SCN_EXECUTE

STEAM = '/mnt/gaming/SteamLibrary/steamapps/common/F1 25/F1_25.exe'
EA = '/mnt/gaming/EAApp/ai-fix/dumps/F1_25.1789128892.110548.mem'
CORE = os.path.join(ROOT, 'native', 'src', 'core')
SOURCES = ['tt_bridge.cpp', 'direct_start.cpp', 'direct_start_tt.cpp', 'lap_rec.cpp', 'era_picker.cpp', 'player_name.cpp']
BASE = 0x140000000

md = Cs(CS_ARCH_X86, CS_MODE_64)
md.detail = True


def strip_comments(text):
    return re.sub(r'//[^\n]*', '', re.sub(r'/\*.*?\*/', '', text, flags=re.S))


def split_args(body):
    out, cur, depth, quoted = [], '', 0, False
    for ch in body:
        if ch == '"':
            quoted = not quoted
        if not quoted and ch in '({':
            depth += 1
        if not quoted and ch in ')}':
            depth -= 1
        if ch == ',' and not quoted and depth == 0:
            out.append(cur.strip())
            cur = ''
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def parse_sigs(path):
    text = strip_comments(open(path).read())
    consts = {m.group(1): ''.join(re.findall(r'"([^"]*)"', m.group(2)))
              for m in re.finditer(r'constexpr\s+const\s+char\s*\*\s*(\w+)\s*=\s*((?:"[^"]*"\s*)+);', text)}
    sigs = []
    for m in re.finditer(r'\bSig\s+(\w+)\s*\{(.*?)\}\s*;', text, flags=re.S):
        a = split_args(m.group(2))
        if len(a) < 3 or not a[0].startswith('"'):
            continue
        pat = ''.join(re.findall(r'"([^"]*)"', a[1])) if '"' in a[1] else consts.get(a[1])
        if pat is None:
            raise SystemExit(f'{path}: {m.group(1)}: pattern {a[1]} is not a literal or a known constant')
        num = lambda i: int(a[i], 0) if len(a) > i else 0
        kind = a[3].split('::')[-1] if len(a) > 3 else 'addr'
        sigs.append({'name': m.group(1), 'what': a[0].strip('"'), 'pattern': ' '.join(pat.split()), 'rva': int(a[2], 0),
                     'kind': kind, 'insn': num(4), 'anchor': num(5), 'len': num(6),
                     'section': a[7].strip('"') if len(a) > 7 else '.xtls', 'file': os.path.basename(path)})
    return sigs


def resolve(img, section, s):
    hits, at, first = img.scan(section, s['pattern'])
    if hits != 1:
        return hits, None, first
    out = at + s['anchor']
    if s['kind'] in ('rel32', 'riprel'):
        out = at + s['insn'] + s['len'] + img.i32(at + s['anchor'])
    return hits, out, at


def norm(img, rva, n):
    """Instructions of [rva, rva+n) with rip displacements and branch/call targets masked."""
    out, code = [], img.read(rva, n + 16) or b''
    for i in md.disasm(code, rva):
        if i.address - rva >= n:
            break
        b = bytearray(i.bytes)
        if i.disp_size and any(o.type == X86_OP_MEM and o.mem.base == X86_REG_RIP for o in i.operands):
            b[i.disp_offset:i.disp_offset + i.disp_size] = b'\0' * i.disp_size
        if (i.group(X86_GRP_JUMP) or i.group(X86_GRP_CALL)) and i.imm_size and any(o.type == X86_OP_IMM for o in i.operands):
            b[i.imm_offset:i.imm_offset + i.imm_size] = b'\0' * i.imm_size
        out.append(bytes(b).hex())
    return out


def ratio(a, b):
    return difflib.SequenceMatcher(None, a, b, autojunk=False).ratio() if a and b else 0.0


def follow(img, rva, hops=4):
    while hops and img.readable(rva, 5) and img.mem[rva] == 0xE9:
        rva, hops = rva + 5 + img.i32(rva + 1), hops - 1
    return rva


def classify(img, rva):
    s = img.section_of(rva)
    if s is None:
        return 'none'
    # A vtable's first words point into code.
    q = struct.unpack_from('<Q', img.mem, rva)[0] - img.image_base if img.readable(rva, 8) else -1
    q2 = struct.unpack_from('<Q', img.mem, rva + 8)[0] - img.image_base if img.readable(rva + 8, 8) else -1
    if 0 < q < img.size and 0 < q2 < img.size and img.in_exec(q) and img.in_exec(q2):
        return 'vtable'
    if not s.flags & SCN_EXECUTE:
        return 'data'
    return 'func' if img.func_base_of(rva) == rva else 'site'


def same(ref, port, rt, pt, kind):
    """(verdict, detail) for two resolved targets."""
    if kind == 'data':
        rs, ps = ref.section_of(rt), port.section_of(pt)
        return 'data', f'sections {rs.name if rs else "?"} / {ps.name if ps else "?"}'
    if kind == 'vtable':
        sims = []
        for k in range(8):
            a = struct.unpack_from('<Q', ref.mem, rt + 8 * k)[0] - ref.image_base
            b = struct.unpack_from('<Q', port.mem, pt + 8 * k)[0] - port.image_base
            if not (0 < a < ref.size and ref.in_exec(a)) or not (0 < b < port.size and port.in_exec(b)):
                break
            sims.append(ratio(norm(ref, follow(ref, a), 0x40), norm(port, follow(port, b), 0x40)))
        ok = len(sims) >= 2 and sum(x >= 0.7 for x in sims) >= len(sims) - 1
        return ('same' if ok else 'DIFFERENT'), f'vtable, {len(sims)} slot(s) alike ' + ' '.join(f'{x:.2f}' for x in sims)
    if kind in ('func', 'site') and ref.mem[rt] == 0xE9 and port.mem[pt] == 0xE9 and not ref.section_named(follow(ref, rt), ref.section_of(rt).name):
        # A protector thunk: a jump out of the code section to the relocated body, then filler. The bodies are compared.
        a, b = norm(ref, follow(ref, rt), 0x60), norm(port, follow(port, pt), 0x60)
        r = ratio(a, b)
        return ('same' if r >= 0.7 else 'DIFFERENT'), f'thunk; relocated body 0x60 alike {r:.2f} ({ref.section_of(follow(ref, rt)).name} / {port.section_of(follow(port, pt)).name})'
    if kind == 'site':
        a, b = norm(ref, rt, 0x20), norm(port, pt, 0x20)
        around = ratio(norm(ref, rt, 0x60), norm(port, pt, 0x60))
        return ('same' if a == b else 'DIFFERENT'), f'hook point 0x20 bytes {"identical" if a == b else "differ"}, 0x60 alike {around:.2f}'
    a, b = norm(ref, follow(ref, rt), 0x60), norm(port, follow(port, pt), 0x60)
    r = ratio(a, b)
    return ('same' if r >= 0.9 else 'likely' if r >= 0.7 else 'DIFFERENT'), f'entry 0x60 alike {r:.2f}'


def section_map(build):
    doc = json.load(open(os.path.join(ROOT, 're', 'db', 'builds', build + '.json')))
    return doc.get('sections', {})


def riprel_refs(img, section, target):
    out = []
    for s in img.sections:
        if s.name != section:
            continue
        lo, hi = s.va, min(s.va + s.size, img.size) - 4
        b = np.frombuffer(img.mem, dtype=np.uint8, count=hi + 4 - lo, offset=lo)
        for st in range(0, hi - lo, 1 << 24):
            en = min(hi - lo, st + (1 << 24))
            seg = b[st:en + 3].astype(np.int64)
            n = en - st
            d = seg[0:n] | (seg[1:n + 1] << 8) | (seg[2:n + 2] << 16) | (seg[3:n + 3] << 24)
            d = np.where(d >= 1 << 31, d - (1 << 32), d)
            pos = np.arange(st, en, dtype=np.int64) + lo
            out += [int(x) for x in pos[(pos + 4 + d) == target]]
    return out


def masked_pattern(img, rva, n):
    pat, code = [], img.read(rva, n + 16)
    for i in md.disasm(code, rva):
        b = [f'{x:02X}' for x in i.bytes]
        if i.disp_size and any(o.type == X86_OP_MEM and o.mem.base == X86_REG_RIP for o in i.operands):
            b[i.disp_offset:i.disp_offset + i.disp_size] = ['??'] * i.disp_size
        if (i.group(X86_GRP_JUMP) or i.group(X86_GRP_CALL)) and i.imm_size >= 4 and any(o.type == X86_OP_IMM for o in i.operands):
            b[i.imm_offset:i.imm_offset + i.imm_size] = ['??'] * i.imm_size
        pat.append(b)
        if sum(map(len, pat)) >= n:
            break
    return pat


def make(ref, port, smap, rva, section='.xtls', back=0, maxlen=96):
    """Shortest instruction-aligned masked pattern from rva-back that matches once in both images."""
    start = rva - back
    insns = masked_pattern(ref, start, maxlen)
    for k in range(1, len(insns) + 1):
        toks = [t for i in insns[:k] for t in i]
        while toks and toks[-1] == '??':
            toks.pop()
        if len(toks) < 10:
            continue
        text = ' '.join(toks)
        h1, a1, _ = ref.scan(section, text)
        if h1 != 1:
            continue
        h2, a2, _ = port.scan(smap.get(section, section), text)
        if h2 == 1:
            return text, a1, a2
    return None, None, None


def twin(ref, port, smap, rva):
    for section in ('.xtls', '.data1'):
        s = ref.section_of(rva)
        if s is None or s.name != section:
            continue
        text, a1, a2 = make(ref, port, smap, rva, section, 0, 160)
        if text:
            return a2
    return None


def dis(img, rva, n):
    for i in md.disasm(img.read(rva, n) or b'', rva):
        print(f'{i.address:8x}: {i.bytes.hex():28s} {i.mnemonic} {i.op_str}')


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--ref', default=STEAM)
    ap.add_argument('--port', default=EA)
    ap.add_argument('--build', default='ea')
    ap.add_argument('--src', nargs='*')
    ap.add_argument('--only')
    ap.add_argument('--json')
    ap.add_argument('--mk')
    ap.add_argument('--kind', default='addr')
    ap.add_argument('--back', type=lambda v: int(v, 0), default=0)
    ap.add_argument('--max', type=lambda v: int(v, 0), default=96)
    ap.add_argument('--section', default='.xtls')
    ap.add_argument('--refs')
    ap.add_argument('--on', default='ref', choices=('ref', 'port'))
    ap.add_argument('--dis', nargs='+')
    ap.add_argument('--twin')
    ap.add_argument('--disp', nargs=3)
    a = ap.parse_args()
    ref, port, smap = Image(a.ref), Image(a.port), section_map(a.build)

    if a.dis:
        img = ref if a.dis[0] == 'ref' else port
        dis(img, int(a.dis[1], 16), int(a.dis[2], 16) if len(a.dis) > 2 else 0x60)
        return 0
    if a.refs:
        t = int(a.refs, 16)
        img = port if a.on == 'port' else ref
        for site in riprel_refs(img, a.section, t):
            f = img.func_base_of(site)
            print(f'{site:#x} (disp at; function {f:#x})' if f else f'{site:#x}')
        return 0
    if a.mk:
        rva = int(a.mk, 16)
        text, a1, a2 = make(ref, port, smap, rva, a.section, a.back, a.max)
        if not text:
            print('no pattern unique on both images')
            return 1
        print(f'pattern: {text}\nref match {a1:#x}  port match {a2:#x}  (anchor {rva - a1:#x})')
        return 0
    if a.twin:
        t = twin(ref, port, smap, int(a.twin, 16))
        print(f'{t:#x}' if t else 'not found')
        return 0 if t else 1
    if a.disp:
        rva, n, want = int(a.disp[0], 16), int(a.disp[1], 16), {int(x, 16) for x in a.disp[2].split(',')}
        t = twin(ref, port, smap, rva)
        for name, img, at in (('ref', ref, rva), ('port', port, t)):
            if at is None:
                print(f'{name}: no twin')
                continue
            seen = {}
            for i in md.disasm(img.read(at, n) or b'', at):
                for o in i.operands:
                    if o.type == X86_OP_MEM and o.mem.base != X86_REG_RIP and o.mem.disp in want:
                        seen.setdefault(o.mem.disp, []).append(f'+{i.address - at:#x} {i.mnemonic} {i.op_str}')
            print(f'{name} {at:#x}:')
            for d in sorted(want):
                print(f'  {d:#x}: {len(seen.get(d, []))} use(s) ' + ' | '.join(seen.get(d, [])[:4]))
        return 0

    sigs = []
    for src in a.src or [os.path.join(CORE, s) for s in SOURCES]:
        sigs += parse_sigs(src)
    if a.only:
        keep = set(a.only.split(','))
        sigs = [s for s in sigs if s['name'] in keep]
    bad, rows = 0, []
    tot = {'sigs': 0, 'ref1': 0, 'port1': 0, 'same': 0, 'blocked': 0}
    for s in sigs:
        psec = smap.get(s['section'], s['section'])
        rh, rt, rm = resolve(ref, s['section'], s)
        ph, pt, pm = resolve(port, psec, s)
        row = dict(s, ref_hits=rh, ref_target=rt, port_hits=ph, port_target=pt, port_section=psec)
        tot['sigs'] += 1
        tot['ref1'] += rh == 1
        tot['port1'] += ph == 1
        notes = []
        if rh == 1 and rt != s['rva']:
            notes.append(f'KB target {rt:#x} is not the declared rva {s["rva"]:#x}')
        verdict, detail = '-', ''
        if rh == 1 and ph == 1:
            kind = classify(ref, rt)
            verdict, detail = same(ref, port, rt, pt, kind)
            if port.page_encrypted(pt) or port.page_encrypted(pm):
                notes.append('port page reads encrypted')
            row['class'] = kind
            tot['same'] += verdict in ('same', 'data')
        ok = rh == 1 and ph == 1 and verdict in ('same', 'data') and not notes
        if s['section'] != '.xtls' and not ok:
            # A hook placed in the protector's section: the core keeps these on the KB build only (see the Sig's use).
            verdict, ok = verdict if verdict != '-' else 'kb-only', rh == 1
            notes.append('protector section: KB build only')
            tot['blocked'] += 1
        elif s['section'] != '.xtls':
            notes, ok = notes + ['protector section: KB build only'], True
            tot['blocked'] += 1
        bad += not ok
        row.update(verdict=verdict, detail=detail, notes=notes, ok=ok)
        rows.append(row)
        print(f'{"ok  " if ok else "FAIL"} {s["name"]:26s} {s["section"]:6s} ref {rh}x {("%#x" % rt) if rt is not None else "-":>10s}  '
              f'port {ph}x {("%#x" % pt) if pt is not None else "-":>10s}  {row.get("class", ""):6s} {verdict:9s} {detail}'
              + (''.join('  !! ' + n for n in notes)))
    print(f'\n{tot["sigs"]} signatures: {tot["ref1"]} unique on ref, {tot["port1"]} unique on port, {tot["same"]} same target, {tot["blocked"]} in the protector section (KB build only); {bad} failing')
    if a.json:
        json.dump(rows, open(a.json, 'w'), indent=1)
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())

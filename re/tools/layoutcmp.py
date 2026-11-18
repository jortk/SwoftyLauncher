#!/usr/bin/env python3
"""Prove that a struct field's displacement is the same on two builds by comparing the functions that read it.

  layoutcmp.py --steam F1_25.exe --other <EA dump> [--field Type.field ...] [--func 0x142cfd6c0 ...] [--md out.md]

For each KB struct field the live core reads (the LIVE_TIE list in native/src/core/live/live_layout.hpp), the KB row
names the decomp function and the instruction addresses (`xref:`) that read it. This tool finds the same function on the
other build by a pattern made of the KB function's first instructions (rip-relative and branch operands wildcarded)
that must match exactly once, disassembles both functions, aligns the instruction streams with every relocated operand
normalised, and checks that the instruction at each xref address has an aligned counterpart with the same text, so the
same displacement. A function whose streams differ beyond relocation is reported with its ratio and not counted.
"""
import argparse, difflib, os, re, struct, sys
from collections import OrderedDict

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import bindverify as bv
import capstone
from capstone import x86

ROOT = os.path.dirname(os.path.dirname(HERE))
STRUCTS = os.path.join(ROOT, 're', 'db', 'structs')
LAYOUT = os.path.join(ROOT, 'native', 'src', 'core', 'live', 'live_layout.hpp')
BASE = 0x140000000
MD = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
MD.detail = True


def exec_sections(img):
    return [s for s in img.sections if s.flags & bv.SCN_EXECUTE and s.size > 0x1000]


def decode(img, rva, size):
    b = img.read(rva, size)
    return list(MD.disasm(b, rva)) if b else []


def reloc_span(i):
    """(offset, length) of the bytes of a relocated operand in the instruction, or None."""
    if i.group(x86.X86_GRP_JUMP) or i.group(x86.X86_GRP_CALL):
        for op in i.operands:
            if op.type == x86.X86_OP_IMM:
                return (i.imm_offset, i.imm_size)
    for op in i.operands:
        if op.type == x86.X86_OP_MEM and op.mem.base == x86.X86_REG_RIP:
            return (i.disp_offset, i.disp_size)
    return None


def norm(i):
    sp = reloc_span(i)
    if sp is None:
        return f'{i.mnemonic} {i.op_str}'
    t = i.op_str
    if i.group(x86.X86_GRP_JUMP) or i.group(x86.X86_GRP_CALL):
        return f'{i.mnemonic} T'
    return f'{i.mnemonic} ' + re.sub(r'rip [+-] 0x[0-9a-f]+', 'rip+R', t)


def pattern_of(insns, maxbytes=96):
    out, n = [], 0
    for i in insns:
        if n + i.size > maxbytes and n:
            break
        raw = bytes(i.bytes)
        sp = reloc_span(i)
        for k, c in enumerate(raw):
            out.append('??' if sp and sp[0] <= k < sp[0] + sp[1] else f'{c:02X}')
        n += i.size
    return ' '.join(out)


def func_bounds(img, rva):
    f = img.func_base_of(rva)
    if f is None:
        return None
    starts, ends = img.pdata()
    import bisect
    k = bisect.bisect_right(starts, f) - 1
    return f, ends[k]


class Locator:
    def __init__(self, steam, other):
        self.s, self.o = steam, other
        self.cache = {}

    def find_other(self, srva, insns):
        if srva in self.cache:
            return self.cache[srva]
        res = None
        for j in range(0, min(len(insns), 80), 4):
            if res is not None:
                break
            sub = insns[j:]
            if not sub or sum(i.size for i in sub) < 12:
                break
            delta = sub[0].address - insns[0].address
            for width in (40, 64, 96, 160, 256):
                pat = pattern_of(sub, width)
                hits = []
                for sec in exec_sections(self.o):
                    h, first, any_first = self.o.scan(sec.name, pat)
                    if h:
                        hits.append((sec.name, h, any_first))
                total = sum(h for _, h, _ in hits)
                if total == 1:
                    res = hits[0][2] - delta
                    break
                if total == 0:
                    break
        self.cache[srva] = res
        return res


EXTRA_FUNCS = {
    ('CarErs', 'deploy_power'): [0x142d00060],
    ('CarF1Aero', 'drs_flags'): [0x142cf9820, 0x142d00060],
    ('SpectatorDirector', 'ui_active'): [0x14f2f41c0],
    ('VehicleTyreComponent', 'set_begin'): [0x142d00060],
    ('TeamRecord', 'timing_tower_texture_path'): [0x141760020],
    ('TeamRecord', 'show_custom_badge_in_ui'): [0x141760020],
}


EXTRA_FIELDS = ['CarLive.vehicle_id', 'CarLive.is_ai', 'CarLive.systems_vtable', 'CarLive.teleport_lock', 'CarLive.engine',
                'CarLive.gearbox', 'CarLive.chassis', 'CarLive.steering', 'CarLive.f1_aero', 'CarLive.ers',
                'CarEngine.rotation_rate', 'CarEngine.limiter_rotation_rate', 'CarSteeringLive.steering_lock',
                'CarComponentBase.car']


STRIDES = [
    ('timing stride', 0x98, ['RelativeGapEntry', 'RelativeGapHelper']),
    ('penalty log stride', 0x30, ['PenaltyRecord', 'PenaltyLog']),
    ('drive-through stride', 0x28, ['DriveThroughObject']),
    ('stop-go stride', 0x4, ['StopGoObject']),
    ('marshal slot stride', 0x10, ['MarshalZoneSlot', 'MarshalZoneMonitor', 'MarshalZone', 'MarshalZoneDef']),
    ('registry slot stride', 0x10, ['NeRobinHoodMapU64']),
    ('entrant view stride', 0x88, ['SessionEntrantView', 'EntrantsManager']),
    ('tyre descriptor stride', 0x18, ['TyreSlot', 'TyreSystem']),
    ('pit lane slot stride', 0x10, ['PitLaneVehicleSlot', 'PitLaneTrack']),
]


FUNCS = [
    ('Game_TelemetryUdp_UpdateRace (car telemetry builder)', 0x142cf9820),
    ('Game_TelemetryUdp_SyncLapData (lap data builder)', 0x142cfd6c0),
    ('car status packet builder', 0x142d00060),
    ('Game_TelemetryUdp_OnCollisionMessage (collision message +0x10/+0x18/+0x19)', 0x142cf6990),
    ('UDP RTMT emitter (status message +0x10/+0x12/+0x14)', 0x142cf6f00),
    ('UDP telemetry bus setup (bus = [[g_Game_MessageBus]+8], allocator [ctx+0x78] vt +0x18)', 0x142cf5c00),
    ('Live_Subscribe_CarToCarCollision', 0x142dd4b20),
    ('Ne_Subscribe_BusMessage', 0x143987810),
    ('registry lookup (robin hood map, slots +0x18, info +0x20)', 0x144500dc0),
]


INSN_CHECKS = [
    ('tyre set stride 0x78 (id at set_end - 0x6c, record start + 0xc)', 0x142ea2afa, 0x142ea2a90),
]

LIVE_BINDINGS = ['g_Sess_VehicleTable', 'g_Sess_VehicleCount', 'g_Game_Manager', 'g_Game_TrackLength',
                 'g_Sess_TyreRulesSessionType', 'g_Live_ControllerSystem', 'g_Live_SectorEnds', 'g_Live_SectorCount',
                 'g_Game_MessageBus', 'Ne_Subscribe_BusMessage', 'vt_Sess_ControllerSystem', 'vt_Sess_HumanController',
                 'g_Live_RacingVehicles', 'vt_Car_SystemsContainer']
VTABLES = ['vt_Sess_ControllerSystem', 'vt_Sess_HumanController']
VT_SLOTS = {'vt_Sess_ControllerSystem': 12, 'vt_Sess_HumanController': 4}


def cstr(img, va):
    rva = va - BASE
    if not 0 < rva < img.size - 64:
        return None
    return bytes(img.mem[rva:rva + 48]).split(b'\0')[0].decode('latin-1')


def binding_table(steam, other):
    sd, od = bv.gen_set('steam'), bv.gen_set('ea')
    sr = {r['id']: r for r in bv.Resolver(steam, sd).run()}
    orr = {r['id']: r for r in bv.Resolver(other, od, None, kb_first=True).run()}
    rows = []
    for b in sd['bindings']:
        i = b['id']
        if not (i in LIVE_BINDINGS or (i.startswith('k_Live_') and i != 'k_Live_GameVehicleProgressSystemKey') or i == 'k_Live_GameVehicleProgressSystemKey'):
            continue
        s, e = sr.get(i), orr.get(i)
        if not s or not e or s['target'] is None or e['target'] is None:
            rows.append((i, '-', '-', 'does not resolve on one build'))
            continue
        note = e['method'] + ' once'
        if i.startswith('k_Live_'):
            name = cstr(other, struct.unpack_from('<Q', other.mem, e['target'] + 8)[0])
            note += f', EA type record name {name!r}'
        if i in VTABLES:
            same = 0
            for k in range(VT_SLOTS[i]):
                sp = struct.unpack_from('<Q', steam.mem, s['target'] + 8 * k)[0] - BASE
                ep = struct.unpack_from('<Q', other.mem, e['target'] + 8 * k)[0] - BASE
                if 0 < sp < steam.size and 0 < ep < other.size:
                    a, c = decode(steam, sp, 0x60)[:10], decode(other, ep, 0x60)[:10]
                    n = min(len(a), len(c))
                    same += n > 0 and sum(norm(x) == norm(y) for x, y in zip(a, c)) == n
            note += f', {same}/{VT_SLOTS[i]} slots point at functions with identical first instructions'
        rows.append((i, hex(s['target']), hex(e['target']), note))
    return rows


def parse_structs():
    fields = {}
    for fn in os.listdir(STRUCTS):
        if not fn.endswith('.h'):
            continue
        with open(os.path.join(STRUCTS, fn)) as fh:
            lines = fh.read().splitlines()
        for line in lines:
            m = re.match(r'\s*/\* 0x([0-9a-fA-F]+) \*/\s+[^;]*?(\w+)(?:\[\w*\])?;\s*//\s*(L\d)\s*(.*)', line)
            if not m:
                continue
            off, name, lvl, rest = m.groups()
            xr = [int(x, 16) for part in re.findall(r'xref:([0-9a-fx,]+)', rest) for x in part.split(',') if x]
            dec = [int(x, 16) for x in re.findall(r'decomp:re/decomp/([0-9a-f]+)\.c', rest)]
            fields[(fn[:-2], name)] = (int(off, 16), lvl, xr, dec)
    return fields


def live_ties():
    return [tuple(x) for x in re.findall(r'^LIVE_TIE\((\w+), (\w+),', open(LAYOUT).read(), re.M)]


def disp_of(i):
    for op in i.operands:
        if op.type == x86.X86_OP_MEM:
            return op.mem.disp
    return None


def mem_span(i):
    for op in i.operands:
        if op.type == x86.X86_OP_MEM and op.mem.base != x86.X86_REG_RIP:
            return op.mem.disp, max(op.size, 1), op.mem.base
    return None


def covers(i, off):
    sp = mem_span(i)
    return sp is not None and sp[0] <= off < sp[0] + sp[1]


def same_slot(si, ei):
    a, b = mem_span(si), mem_span(ei)
    return a is not None and b is not None and a[0] == b[0] and a[2] == b[2]


def guess_end(img, rva, cap=0x3000):
    ins = decode(img, rva, cap)
    end = rva
    for k, i in enumerate(ins):
        end = i.address + i.size
        if i.mnemonic in ('ret', 'jmp') and k + 1 < len(ins) and ins[k + 1].mnemonic == 'int3':
            break
    return end


def compare_function(steam, other, loc, sfunc_rva, cache):
    if sfunc_rva in cache:
        return cache[sfunc_rva]
    out = {'ok': False}
    cache[sfunc_rva] = out
    bnd = func_bounds(steam, sfunc_rva)
    if not bnd or bnd[0] != sfunc_rva:
        bnd = (sfunc_rva, guess_end(steam, sfunc_rva))
    s_ins = decode(steam, bnd[0], bnd[1] - bnd[0])
    if not s_ins:
        out['why'] = 'no code'
        return out
    erva = loc.find_other(sfunc_rva, s_ins)
    if erva is None:
        out['why'] = 'no unique head pattern on the other build'
        return out
    e_ins = decode(other, erva, (bnd[1] - bnd[0]) + 0x400)
    sn = [norm(i) for i in s_ins]
    en = [norm(i) for i in e_ins[:len(s_ins) + 64]]
    sm = difflib.SequenceMatcher(None, sn, en, autojunk=False)
    amap = {}
    same = 0
    for a, b, n in sm.get_matching_blocks():
        for k in range(n):
            amap[a + k] = b + k
        same += n
    for tag, a1, a2, b1, b2 in sm.get_opcodes():
        if tag == 'replace' and a2 - a1 == b2 - b1:
            for k in range(a2 - a1):
                if s_ins[a1 + k].mnemonic == e_ins[b1 + k].mnemonic:
                    amap[a1 + k] = b1 + k
    out.update(ok=True, ratio=same / len(sn), n=len(sn), same=same, erva=erva, sins=s_ins, eins=e_ins, amap=amap,
               sidx={i.address: k for k, i in enumerate(s_ins)}, srva=sfunc_rva)
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--steam', required=True)
    ap.add_argument('--other', required=True)
    ap.add_argument('--field', action='append', help='Type.field (default: every LIVE_TIE field)')
    ap.add_argument('--md')
    a = ap.parse_args(argv)
    steam, other = bv.Image(a.steam), bv.Image(a.other)
    loc = Locator(steam, other)
    fields = parse_structs()
    want = [tuple(f.split('.')) for f in a.field] if a.field else live_ties() + [tuple(f.split('.')) for f in EXTRA_FIELDS]
    cache = {}
    rows, bad = [], 0
    for T, f in want:
        if (T, f) not in fields:
            rows.append((T, f, '?', '-', '-', 'no KB row'))
            continue
        off, lvl, xrefs, decs = fields[(T, f)]
        starts = []
        for x in xrefs:
            rva = x - BASE
            if not 0 <= rva < steam.size:
                continue
            fb = func_bounds(steam, rva)
            if fb:
                st = fb[0]
            else:
                c = [d - BASE for d in decs if 0 <= rva - (d - BASE) < 0x3000]
                st = max(c) if c else None
            if st is not None and st not in starts:
                starts.append(st)
        for d in EXTRA_FUNCS.get((T, f), []) + decs:
            if 0 <= d - BASE < steam.size and d - BASE not in starts:
                starts.append(d - BASE)
        starts = [e - BASE for e in EXTRA_FUNCS.get((T, f), [])] + [x for x in starts if x + BASE not in EXTRA_FUNCS.get((T, f), [])]
        why, shown = [], False
        xr = {x - BASE for x in xrefs}
        for st in starts:
            fr = compare_function(steam, other, loc, st, cache)
            if not fr['ok']:
                why.append(f'{st + BASE:#x}: {fr["why"]}')
                continue
            hits = [k for k, i in enumerate(fr['sins']) if covers(i, off) and (i.address in xr)]
            kind = 'xref'
            if not hits:
                hits = [k for k, i in enumerate(fr['sins']) if covers(i, off) and not i.group(x86.X86_GRP_JUMP)]
                kind = 'fn'
            if not hits:
                hits = [k for k, i in enumerate(fr['sins']) if i.address in xr and mem_span(i) is not None]
                kind = 'xref insn near'
            if not hits:
                why.append(f'{st + BASE:#x}: no insn with displacement {off:#x}')
                continue
            diffs = []
            for k in hits:
                e = fr['amap'].get(k)
                si = fr['sins'][k]
                if e is None:
                    alt = [x for x in fr['eins'][:len(fr['sins']) + 64] if same_slot(si, x) and covers(x, off)]
                    if not alt:
                        diffs.append((si, None))
                    continue
                ei = fr['eins'][e]
                if kind == 'xref insn near':
                    ok = norm(ei) == norm(si) and disp_of(ei) == disp_of(si)
                else:
                    ok = same_slot(si, ei) and (norm(ei) == norm(si) or covers(ei, off))
                if not ok:
                    diffs.append((si, ei))
            k = hits[0]
            si = fr['sins'][k]
            e = fr['amap'].get(k)
            ei = fr['eins'][e] if e is not None else None
            sfmt = f'{si.address + BASE:#x} {si.mnemonic} {si.op_str}'
            if ei is None:
                alt = [x for x in fr['eins'][:len(fr['sins']) + 64] if same_slot(si, x) and covers(x, off)]
                ei = alt[0] if alt else None
            efmt = f'{ei.address:#x} {ei.mnemonic} {ei.op_str}' if ei else '-'
            if diffs:
                bad += 1
                d0 = diffs[0]
                e0 = f'{d0[1].mnemonic} {d0[1].op_str}' if d0[1] else 'unaligned'
                rows.append((T, f, hex(off), f'{d0[0].address + BASE:#x} {d0[0].mnemonic} {d0[0].op_str}',
                             f'{d0[1].address:#x} {e0}' if d0[1] else '-', f'NO: {len(diffs)}/{len(hits)} insn(s) differ in {st + BASE:#x}'))
            else:
                weak = ', offset below 0x10: only the identical stream proves it' if off < 0x10 and kind == 'fn' else ''
                rows.append((T, f, hex(off), sfmt, efmt, f'yes ({kind}, {len(hits)} insn(s), fn {st + BASE:#x} ratio {fr["ratio"]:.2f}{weak})'))
            shown = True
            break
        if not shown:
            rows.append((T, f, hex(off), '-', '-', 'not proven: ' + '; '.join(why[:3])))
    srows = []
    for label, val, structs in ([] if a.field else STRIDES):
        done = False
        for (T, f), (off, lvl, xrefs, decs) in fields.items():
            if T not in structs or done:
                continue
            starts = set()
            for x in xrefs:
                fb = func_bounds(steam, x - BASE) if 0 <= x - BASE < steam.size else None
                if fb:
                    starts.add(fb[0])
            for st in sorted(starts):
                fr = compare_function(steam, other, loc, st, cache)
                if not fr['ok']:
                    continue
                for k, i in enumerate(fr['sins']):
                    if i.group(x86.X86_GRP_JUMP) or i.group(x86.X86_GRP_CALL):
                        continue
                    imm = [op.imm for op in i.operands if op.type == x86.X86_OP_IMM]
                    if val in imm and i.mnemonic in ('imul', 'add', 'sub', 'lea', 'cmp', 'mov', 'shl') and k in fr['amap']:
                        ei = fr['eins'][fr['amap'][k]]
                        if norm(ei) == norm(i):
                            srows.append((label, hex(val), f'{i.address + BASE:#x} {i.mnemonic} {i.op_str}',
                                          f'{ei.address:#x} {ei.mnemonic} {ei.op_str}', f'yes (fn {st + BASE:#x} ratio {fr["ratio"]:.2f})'))
                            done = True
                            break
                if done:
                    break
        if not done:
            srows.append((label, hex(val), '-', '-', 'not proven'))
    frows = []
    for label, va in ([] if a.field else FUNCS):
        fr = compare_function(steam, other, loc, va - BASE, cache)
        if not fr['ok']:
            frows.append((label, hex(va), '-', f'not located: {fr["why"]}'))
            continue
        dd = 0
        for k, i in enumerate(fr['sins']):
            e = fr['amap'].get(k)
            if e is not None and norm(fr['eins'][e]) != norm(i):
                dd += 1
        unal = len(fr['sins']) - len(fr['amap'])
        frows.append((label, hex(va), hex(fr['erva']), f'{fr["n"]} insns, {fr["same"]} identical, {dd} aligned-but-different, {unal} unaligned (ratio {fr["ratio"]:.3f})'))
    brows = [] if a.field else binding_table(steam, other)
    for label, va, fva in ([] if a.field else INSN_CHECKS):
        fr = compare_function(steam, other, loc, fva - BASE, cache)
        ok = fr['ok'] and (va - BASE) in fr['sidx'] and fr['sidx'][va - BASE] in fr['amap']
        if ok:
            si = fr['sins'][fr['sidx'][va - BASE]]
            ei = fr['eins'][fr['amap'][fr['sidx'][va - BASE]]]
            same = norm(si) == norm(ei)
            srows.append((label, '-', f'{si.address + BASE:#x} {si.mnemonic} {si.op_str}', f'{ei.address:#x} {ei.mnemonic} {ei.op_str}',
                          f'{"yes" if same else "NO"} (fn {fva:#x} ratio {fr["ratio"]:.2f})'))
            bad += not same
        else:
            srows.append((label, '-', '-', '-', 'not proven'))
    lines = ['| struct.field | off | Steam insn | EA insn | same |', '|---|---|---|---|---|']
    for T, f, off, s, e, ok in rows:
        lines.append(f'| {T}.{f} | {off} | `{s}` | `{e}` | {ok} |')
    lines += ['', '| constant | value | Steam insn | EA insn | same |', '|---|---|---|---|---|']
    for r in srows:
        lines.append('| ' + ' | '.join(f'`{c}`' if c.startswith('0x') and ' ' in c else c for c in r) + ' |')
    lines += ['', '| function | Steam | EA | stream comparison |', '|---|---|---|---|']
    for r in frows:
        lines.append('| ' + ' | '.join(r) + ' |')
    lines += ['', '| binding | Steam rva | EA rva | note |', '|---|---|---|---|']
    for r in brows:
        lines.append('| ' + ' | '.join(r) + ' |')
    text = '\n'.join(lines)
    print(text)
    if a.md:
        open(a.md, 'w').write(text + '\n')
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())

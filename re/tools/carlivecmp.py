#!/usr/bin/env python3
"""Check every offset, call, site and global the live car operations use on another build of F1_25.exe.

  carlivecmp.py --kb <KB F1_25.exe> --other <exe or flat dump> --build <overlay> [--md out.md]

native/src/core/hooks/car_live.{hpp,cpp} read and write fields of CarLive and its systems, call game functions after a
write, hook game points and mid-hook sites, and read the EngineModeAvail global. This tool checks each of them on the
other build against the KB build:

- code: every KB function that reads or writes a field (EVIDENCE) is found on the other build by layoutcmp.py's head
  pattern and has to be instruction-identical there with relocated operands normalised; an identical function carries
  the same displacements;
- VTF readers: a system's section reader is reached from its identical constructor (data vtable at +0x18, slot 2,
  through the protector's jmp thunk), and the VTF key name each read passes is paired with the field offset it fills
  (value and key 8 bytes on for an obfuscated float); the pairs have to be equal;
- bodies: the protected bodies car_live calls or hooks through a thunk are compared instruction by instruction, and
  the instructions that differ are listed with the opaque constants they compute from the image's data;
- sites: the function holding each mid-hook site is identical and the site lands on the binding's address;
- globals: the KB writer of g_Game_EngineModeAvail is aligned on the other build and gives that build's address;
- bindings: every binding car_live resolves resolves on the other build (bindverify.py) and, where code reaches it,
  is the address that code calls.
"""
import argparse, difflib, os, re, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import layoutcmp as lc
from capstone import x86

bv = lc.bv
B = lc.BASE

EVIDENCE = [
    ('CarLive.vehicle_id +0xe8, CarLive.upgrade_loader +0x4a0 (Car_Chassis_Build)', 0x144628190, [0xe8, 0x4a0]),
    ('CarLive system slots +0x530..+0x5d0 (Car_Build_VtfComponents)', 0x14473c2e0,
     [0x530, 0x538, 0x540, 0x548, 0x550, 0x558, 0x560, 0x570, 0x578, 0x580, 0x5a0, 0x5c8, 0x5d0]),
    ('CarLive.is_ai +0x124 (Car_Dynamics_Update)', 0x1446dc480, [0x124]),
    ('CarComponentBase.car +0x8 (component base constructor)', 0x1447484f0, [0x8]),
    ('CarSteering.max_steering_lock +0x28 obf (UDP car telemetry)', 0x142cf9820, [0x28, 0x30]),
    ('CarGearBox final drive, ratios, clutch (VTF GearBox reader)', 0x1446a39a0,
     [0x10, 0x18, 0x58, 0x60, 0x68, 0xd8, 0xe0, 0xe8]),
    ('CarGearBox.use_setup_ratios +0x540 (Car_Tuning_SetFinalDrive)', 0x1446a4c20, [0x540]),
    ('CarFuelTank.fuel_mass +0x100 (Car_FuelTank_StepBurn)', 0x14468edb0, [0x100]),
    ('CarDamageFrontWing fields, wing +0x08/+0x48/+0x4c (Car_FrontWing_OnCollision)', 0x14465fdc0,
     [0x8, 0x28, 0x2c, 0x30, 0x38, 0x48, 0x4c]),
    ('CarDamageFrontWing reader', 0x14465f450, [0x10, 0x14, 0x18, 0x1c, 0x20]),
    ('CarDamageFrontWing side/full detach (wing detach)', 0x144631a50, [0x34, 0x38]),
    ('Car_FrontWing_Init', 0x14465f620, [0x8]),
    ('CarDamageSuspension fields (reader)', 0x144653070, [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x64]),
    ('CarDamageSuspension fields (collision)', 0x1446533e0, [0x28, 0x38, 0x48, 0x58, 0x68, 0x78]),
    ('CarEngine.vtf_limiter_rotation_rate +0x8b0 obf', 0x144685680, [0x8b0, 0x8b8]),
    ('CarF1Aero lift +0x19a4, drag +0x19a0, CarSuspAxle.ride_height_offset +0x15e4', 0x1446dceb0,
     [0x19a0, 0x19a4, 0x15e4]),
    ('CarF1Aero slipstream scales +0xe8..+0x128 obf', 0x1446c8560, [0xd0, 0xe0, 0xf0, 0x100, 0x110]),
    ('CarBrakeEcu.brake_bias +0x21d8, brake_pressure +0x21dc', 0x144676250, [0x21d8, 0x21dc]),
    ('CarSuspension.tyre_types +0x38, CarSuspTyreType.name/grip_multiplier', 0x1446aa090, [0x38, 0x2c]),
    ('CarSuspension.axle0 +0x90, axle1 +0x98', 0x144753050, [0x90, 0x98]),
    ('CarSuspAxle rest z +0x1598, camber +0x1650, toe +0x1654, anti-roll +0x1664', 0x1446a7be0,
     [0x1598, 0x1650, 0x1654, 0x1664]),
    ('Chassis OnReset (system vtable +0x18, flags 0x208, sqrt_mass)', 0x144676d40, []),
    ('Game_Pitlane_SetVehicleStatus: status table +0x68, state +0x4, stops +0x14', 0x1434bc540, [0x68, 0x4, 0x14]),
    ('Car_Upgrades_ApplyToVehicle', 0x144742c70, []),
    ('Car_Tyre_SetWearFactor', 0x142ea2300, []),
    ('Game_Get_ModelNodeId', 0x143a3d780, []),
]

AI_EVIDENCE = [0x1405f5dd0, 0x1405ff750, 0x1406068a0, 0x140606fc0, 0x14060d720, 0x140624e60, 0x14063e370, 0x140650150,
               0x1406527c0, 0x140658ac0, 0x14065da00, 0x14065dad0, 0x14065dc50, 0x14065de80, 0x14065e040, 0x14065e120,
               0x1406663d0, 0x140667f40, 0x14249a4f0]

# System constructor, the VTF keys whose offsets car_live uses, and whether the system vtable's Reset (+0x18) body is
# compared as well.
READERS = [
    ('FuelECU', 0x14468a2b0, ['FuelBurnScale' + m for m in ('Lean', 'Standard', 'Rich', 'Max')] +
     ['FuelPowerScale' + m for m in ('Lean', 'Standard', 'Rich', 'Max')] +
     ['FuelRevsScale' + m for m in ('Lean', 'Standard', 'Rich', 'Max')] +
     ['TemperatureScale' + m for m in ('Lean', 'Standard', 'Rich', 'Max')], True),
    ('Chassis', 0x14474c060, ['Mass'], False),
    ('Steering', 0x14474e650, ['MaxSteeringLock'], False),
    ('FuelTank', 0x144749420, ['FuelCapacity'], False),
    ('Turbo', 0x1555ded40, ['MaxBoost'], False),
    ('Differential', 0x1447518c0, [], True),
]

# The fields a system's Reset body writes (diff_off_throttle and diff_on_throttle are derived there) and the fuel mix
# Car_FuelEcu_SetMode_Thunk and the FuelECU Reset store; each has to sit in an instruction identical on both builds.
BODY_FIELDS = {'Differential Reset (system vtable +0x18)': [0xec, 0xf0], 'FuelECU Reset (system vtable +0x18)': [0x128],
               'Car_FuelEcu_SetMode_Thunk': [0x128]}

BODIES = ['Car_FuelEcu_SetMode_Thunk', 'Car_Engine_ComputeDerived_Thunk', 'Car_Systems_Reset_Thunk',
          'Car_Register_DetachablePart_Thunk', 'Car_Tuning_SetFinalDrive', 'Car_Tuning_SetChassisZCom',
          'Car_FrontWing_Init', 'Car_FrontWing_OnCollision', 'Car_Upgrades_ApplyToVehicle',
          'Game_Pitlane_SetVehicleStatus', 'Car_Tyre_SetWearFactor', 'Game_Get_ModelNodeId']

SITES = ['site_Car_LiveCarBuilt', 'site_Car_SetupTuningApplied', 'site_Car_EngineDriveTorque', 'site_Ai_TableGripInput',
         'site_Ai_BuildSystem_RecordPushed']

BINDINGS = ['g_Sess_VehicleTable', 'g_Sess_VehicleCount'] + BODIES + SITES

# The KB writer of g_Game_EngineModeAvail (session setup 0x1407e5420) and the reader that tests a fuel mix's
# availability before Car_FuelEcu_SetMode_Thunk (Sess_ControllerSystem_ApplyRecord 0x14389a2b0).
MODE_AVAIL_WRITER = (0x1407e5420, [0x1407e5e1e, 0x1407e5ec4, 0x1407e5ece, 0x1407e5ed2, 0x1407e5eda])
MODE_AVAIL_READER = (0x14389a2b0, [0x14389b83c, 0x14389babf, 0x14389bac7, 0x14389bad4, 0x14389badb, 0x14389bbf3])


R64 = {f'e{r}': f'r{r}' for r in ('ax', 'bx', 'cx', 'dx', 'si', 'di', 'bp', 'sp')}
R64.update({f'r{n}d': f'r{n}' for n in range(8, 16)})


def q(img, va):
    return struct.unpack('<Q', img.read(va - B, 8))[0]


def d(img, va):
    return struct.unpack('<I', img.read(va - B, 4))[0]


def cstr(img, va):
    b = img.read(va - B, 64)
    return b.split(b'\0')[0].decode('latin-1') if b else None


def rip_of(i):
    for op in i.operands:
        if op.type == x86.X86_OP_MEM and op.mem.base == x86.X86_REG_RIP:
            return i.address + i.size + op.mem.disp + B
    return None


def branch_of(i):
    if i.group(x86.X86_GRP_JUMP) or i.group(x86.X86_GRP_CALL):
        for op in i.operands:
            if op.type == x86.X86_OP_IMM:
                return op.imm + B
    return None


def through_thunk(img, va):
    i = lc.decode(img, va - B, 16)
    if i and i[0].mnemonic == 'jmp' and branch_of(i[0]):
        return branch_of(i[0])
    return va


def disps(insns):
    out = set()
    for i in insns:
        for op in i.operands:
            if op.type == x86.X86_OP_MEM and op.mem.base not in (x86.X86_REG_RIP, x86.X86_REG_RSP, x86.X86_REG_RBP, 0):
                out.add(op.mem.disp)
    return out


class Check:
    def __init__(self, kb, other):
        self.kb, self.o = kb, other
        self.loc = lc.Locator(kb, other)
        self.cache = {}
        self.rows = []
        self.failed = 0

    def row(self, ok, what, detail):
        self.rows.append((ok, what, detail))
        if ok is False:
            self.failed += 1

    def same_fn(self, va):
        """(compare_function result, (differing, unaligned) KB instructions) with the instructions a hook in a dump
        replaced left out: a dump taken with the core loaded carries its mid-hook patches, a jmp out of the image."""
        fr = lc.compare_function(self.kb, self.o, self.loc, va - B, self.cache)
        if not fr['ok']:
            return fr, None
        if 'patched' not in fr:
            spans = []
            ein = fr['eins']
            for k, i in enumerate(ein):
                t = branch_of(i)
                if i.mnemonic == 'jmp' and t is not None and not 0 <= t - B < self.o.size:
                    end = i.address + i.size
                    for j in ein[k + 1:k + 12]:
                        if j.mnemonic not in ('nop', 'int3'):
                            break
                        end = j.address + j.size
                    spans.append((i.address, end))
            prev = None
            nd = ua = patched = 0
            for k, i in enumerate(fr['sins']):
                e = fr['amap'].get(k)
                if e is not None:
                    prev = (i.address, fr['eins'][e].address)
                    if lc.norm(fr['eins'][e]) == lc.norm(i):
                        continue
                at = fr['eins'][e].address if e is not None else (prev[1] + i.address - prev[0] if prev else None)
                if at is not None and any(a <= at < b + 16 for a, b in spans):
                    patched += 1
                elif e is None:
                    ua += 1
                else:
                    nd += 1
            fr['patched'], fr['diff'] = patched, (nd, ua)
        return fr, fr['diff']

    def evidence(self):
        for what, va, want in EVIDENCE:
            fr, diff = self.same_fn(va)
            if diff is None:
                self.row(False, what, f'{va:#x}: {fr["why"]}')
                continue
            have = disps(fr['sins'])
            missing = [hex(x) for x in want if x not in have]
            ok = diff == (0, 0) and not missing
            self.row(ok, what, f'{va:#x} -> {fr["erva"] + B:#x}: {fr["n"]} instructions, {diff[0]} differ, '
                               f'{diff[1]} unaligned' + (f', {fr["patched"]} under a hook in the dump' if fr['patched'] else '') +
                               (f'; KB function lacks {missing}' if missing else ''))
        n = 0
        bad = []
        for va in AI_EVIDENCE:
            fr, diff = self.same_fn(va)
            if diff != (0, 0):
                bad.append(hex(va))
            else:
                n += fr['n']
        self.row(not bad, 'AiSystem, AiCarRecord and AiDriverConfig evidence functions',
                 f'{len(AI_EVIDENCE) - len(bad)} of {len(AI_EVIDENCE)} identical ({n} instructions)' +
                 (f'; not identical: {", ".join(bad)}' if bad else ''))

    def vtf_pairs(self, img, va, n=0x1800):
        """{VTF key name: (field offset, plain or obf)} of a section reader. A read passes the key name in rdx and the
        destination in r8 (the data object, system+0x18, plus the field's offset, or a stack temporary that an obf
        field is xored from with its key 8 bytes on); the protector may compute the offset as an opaque constant."""
        name = r8 = pend = lastkey = None
        out, regs = {}, {}
        insns = list(lc.MD.disasm(img.read(va - B, n), va - B))
        for k, i in enumerate(insns):
            if i.mnemonic == 'mov' and i.operands and i.operands[0].type == x86.X86_OP_REG:
                v = self.opaque_in(img, insns, k)
                r = R64.get(i.reg_name(i.operands[0].reg), i.reg_name(i.operands[0].reg))
                if v is not None:
                    regs[r] = v
                else:
                    regs.pop(r, None)
            if i.mnemonic == 'lea' and len(i.operands) == 2 and i.operands[1].type == x86.X86_OP_MEM:
                r, m = i.reg_name(i.operands[0].reg), i.operands[1].mem
                if r == 'rdx' and m.base == x86.X86_REG_RIP:
                    name = cstr(img, rip_of(i))
                    continue
                if r == 'r8':
                    if m.base in (x86.X86_REG_RSP, x86.X86_REG_RBP):
                        r8 = 'tmp'
                    elif m.index:
                        r8 = regs.get(i.reg_name(m.index))
                        r8 = None if r8 is None else r8 + m.disp
                    else:
                        r8 = m.disp
                    continue
            if i.mnemonic == 'call':
                if name is not None and r8 is not None:
                    if r8 == 'tmp':
                        pend = name
                    else:
                        out[name] = (r8 + 0x18, 'plain')
                name = r8 = None
                regs.clear()
                continue
            x = re.match(r'e\w\w, dword ptr \[\w+ \+ (0x[0-9a-f]+)\]', i.op_str)
            if i.mnemonic == 'xor' and x:
                lastkey = int(x.group(1), 16)
            m = re.match(r'dword ptr \[(\w+) \+ (0x[0-9a-f]+)\], e\w\w', i.op_str)
            if i.mnemonic == 'mov' and m and pend and m.group(1) not in ('rsp', 'rbp'):
                off = int(m.group(2), 16)
                out[pend] = (off + 0x18, 'obf' if lastkey == off + 8 else 'unkeyed')
                pend = None
            if i.mnemonic == 'ret':
                break
        return out

    def name_of(self, img, fn):
        i = lc.decode(img, fn - B, 16)
        return cstr(img, rip_of(i[0])) if i and i[0].mnemonic == 'lea' and rip_of(i[0]) else None

    def vtables(self, img, insns):
        sys_vt = data_vt = None
        last = {}
        for i in insns[:80]:
            if i.mnemonic == 'lea' and len(i.operands) == 2 and rip_of(i):
                last[i.reg_name(i.operands[0].reg)] = rip_of(i)
            m = re.match(r'qword ptr \[(\w+)(?: \+ (0x18))?\], (\w+)$', i.op_str)
            if i.mnemonic == 'mov' and m and m.group(3) in last:
                if m.group(2):
                    data_vt = data_vt or last[m.group(3)]
                else:
                    sys_vt = sys_vt or last[m.group(3)]
        return sys_vt, data_vt

    def readers(self):
        for name, ctor, keys, reset in READERS:
            fr, diff = self.same_fn(ctor)
            if diff != (0, 0):
                self.row(False, f'{name} constructor', f'{ctor:#x}: {fr.get("why") or diff}')
                continue
            (ks, kd), (os_, od) = self.vtables(self.kb, fr['sins']), self.vtables(self.o, fr['eins'])
            if not kd or not od:
                self.row(False, f'{name} data vtable', 'not found in the constructor')
                continue
            kn, on = self.name_of(self.kb, q(self.kb, kd + 8)), self.name_of(self.o, q(self.o, od + 8))
            kr, orr = through_thunk(self.kb, q(self.kb, kd + 16)), through_thunk(self.o, q(self.o, od + 16))
            kp, op_ = self.vtf_pairs(self.kb, kr), self.vtf_pairs(self.o, orr)
            differ = sorted(k for k in set(kp) | set(op_) if kp.get(k) != op_.get(k))
            missing = [k for k in keys if k not in kp]
            self.row(kn == on == name and bool(kp) and not differ and not missing,
                     f'{name} VTF reader: {len(kp)} keys' + (f', {", ".join(keys)}' if 0 < len(keys) < 4 else
                                                            f', the {len(keys)} mode table keys' if keys else ''),
                     f'data vtable {kd:#x} -> {od:#x} ("{on}"), reader {kr:#x} -> {orr:#x}; '
                     f'{len(kp) - len(differ)} key/offset pairs equal; ' +
                     ', '.join(f'{k} +{kp[k][0]:#x} {kp[k][1]}' for k in keys if k in kp) +
                     (f'; KB reader lacks {missing}' if missing else '') +
                     (f'; DIFFER {differ}: KB {[kp.get(k) for k in differ]} other {[op_.get(k) for k in differ]}'
                      if differ else ''))
            if reset and ks and os_:
                self.body(f'{name} Reset (system vtable +0x18)', q(self.kb, ks + 0x18), q(self.o, os_ + 0x18))

    def opaque_in(self, img, insns, k):
        """The value an opaque constant pair at insns[k] leaves in its register: mov r, imm then add, sub or xor r with
        a dword of the image's data, or the dword loaded first and the immediate applied second."""
        if k + 1 >= len(insns):
            return None
        i0, i1 = insns[k], insns[k + 1]
        ops = {'sub': lambda a, b: a - b, 'add': lambda a, b: a + b, 'xor': lambda a, b: a ^ b}
        try:
            if i1.mnemonic not in ops or i0.operands[0].type != x86.X86_OP_REG or i1.operands[0].type != x86.X86_OP_REG or \
                    i0.operands[0].reg != i1.operands[0].reg:
                return None
            if i0.mnemonic == 'mov' and i0.operands[1].type == x86.X86_OP_IMM and rip_of(i1):
                return ops[i1.mnemonic](i0.operands[1].imm & 0xffffffff, d(img, rip_of(i1))) & 0xffffffff
            if i0.mnemonic == 'mov' and rip_of(i0) and i1.operands[1].type == x86.X86_OP_IMM:
                return ops[i1.mnemonic](d(img, rip_of(i0)), i1.operands[1].imm & 0xffffffff) & 0xffffffff
        except Exception:
            return None
        return None

    def opaque(self, insns, k):
        return self.opaque_in(self.o, insns, k)

    def body(self, what, kva, ova):
        ka, oa = through_thunk(self.kb, kva), through_thunk(self.o, ova)
        fb = lc.func_bounds(self.kb, ka - B)
        end = fb[1] if fb and fb[0] == ka - B else lc.guess_end(self.kb, ka - B)
        si = lc.decode(self.kb, ka - B, end - (ka - B))
        ei = lc.decode(self.o, oa - B, end - (ka - B) + 0x400)[:len(si) + 64]
        sn, en = [lc.norm(i) for i in si], [lc.norm(i) for i in ei]
        sm = difflib.SequenceMatcher(None, sn, en, autojunk=False)
        same = sum(n for _, _, n in sm.get_matching_blocks())
        held = disps([si[a + k] for a, _, n in sm.get_matching_blocks() for k in range(n)])
        lacks = [hex(x) for x in BODY_FIELDS.get(what, []) if x not in held]
        notes = []
        for tag, a1, a2, b1, b2 in sm.get_opcodes():
            if tag == 'equal' or a1 >= len(si):
                continue
            ks, os_ = sn[a1:a2], en[b1:b2]
            if all(t.startswith(('nop', 'int3')) for t in ks + os_):
                continue
            vals = [v for v in (self.opaque(ei, k) for k in range(max(0, b1 - 1), b2)) if v is not None]
            kvals = [v for v in (self.opaque_kb(si, k) for k in range(max(0, a1 - 1), a2)) if v is not None]
            notes.append(f'[{"; ".join(ks[:4])}] -> [{"; ".join(os_[:4])}]' +
                         (f' (other build opaque constant {", ".join(hex(v) for v in vals)})' if vals else '') +
                         (f' (KB opaque constant {", ".join(hex(v) for v in kvals)})' if kvals else ''))
        want = BODY_FIELDS.get(what, [])
        self.row(False if lacks else True if same == len(si) else None, f'{what} body',
                 f'{ka:#x} -> {oa:#x}: {len(si)} instructions, {same} identical' +
                 (f'; fields {", ".join(hex(x) for x in want)} in identical instructions' if want and not lacks else '') +
                 (f'; fields {lacks} NOT in an identical instruction' if lacks else '') +
                 ('; differences to review: ' + ' | '.join(notes) if notes else ''))

    def opaque_kb(self, insns, k):
        return self.opaque_in(self.kb, insns, k)

    def bindings(self, build):
        doc_kb, doc_o = bv.gen_set('steam'), bv.gen_set(build)
        rk = {r['id']: r for r in bv.Resolver(self.kb, doc_kb).run()}
        exe = None if self.o.flat else os.path.getsize(self.opath)
        ro = {r['id']: r for r in bv.Resolver(self.o, doc_o, exe).run()}
        self.bound = {}
        for b in BINDINGS:
            k, o = rk.get(b), ro.get(b)
            ok = bool(k and o and k.get('target') is not None and o.get('target') is not None)
            if ok:
                self.bound[b] = (k['target'] + B, o['target'] + B)
            self.row(ok, f'binding {b}', f'{k["target"] + B:#x} -> {o["target"] + B:#x} ({o["method"]})' if ok else
                     'does not resolve')
        for b in BODIES:
            if b in self.bound:
                self.body(b, *self.bound[b])

    def sites(self):
        for s in SITES:
            if s not in self.bound:
                continue
            kva, ova = self.bound[s]
            fb = lc.func_bounds(self.kb, kva - B)
            fr, diff = self.same_fn(fb[0] + B)
            if diff is None:
                self.row(False, f'{s} function', fr['why'])
                continue
            k = fr['sidx'].get(kva - B)
            e = fr['amap'].get(k) if k is not None else None
            lands = fr['eins'][e].address + B if e is not None else None
            hooked = False
            if lands is None:
                p = next((kk for kk in range(k - 1, -1, -1) if kk in fr['amap']), None)
                if p is not None:
                    est = fr['eins'][fr['amap'][p]].address + (fr['sins'][k].address - fr['sins'][p].address) + B
                    j = lc.decode(self.o, est - B, 16)
                    if j and j[0].mnemonic == 'jmp' and branch_of(j[0]) and not 0 <= branch_of(j[0]) - B < self.o.size:
                        lands, hooked = est, True
            window = [kk for kk in range(max(0, k - 12), min(len(fr['sins']), k + 4))
                      if not (hooked and fr['sins'][kk].address >= fr['sins'][k].address and
                              fr['sins'][kk].address < fr['sins'][k].address + 5)]
            local = all(kk in fr['amap'] and lc.norm(fr['sins'][kk]) == lc.norm(fr['eins'][fr['amap'][kk]]) for kk in window)
            ok = lands == ova and local
            self.row(ok, f'{s} (mid-hook site)',
                     f'function {fb[0] + B:#x} -> {fr["erva"] + B:#x} ({fr["n"]} instructions, {diff[0]} differ, '
                     f'{diff[1]} unaligned; the instructions around the site identical: {local}); site -> '
                     f'{lands and hex(lands)}' + (' (a hook jmp in the dump)' if hooked else '') + f', binding {ova:#x}')

    def aligned(self, fn, insns):
        fr, diff = self.same_fn(fn)
        out = []
        if diff is None:
            return None, out
        for va in insns:
            k = fr['sidx'].get(va - B)
            e = fr['amap'].get(k) if k is not None else None
            if e is None:
                out.append((va, None, None, None))
                continue
            i, j = fr['sins'][k], fr['eins'][e]
            out.append((va, j.address + B, lc.norm(i) == lc.norm(j), (rip_of(i), rip_of(j), branch_of(i), branch_of(j))))
        return (fr, diff), out

    def globals(self):
        (fr, diff), rows = self.aligned(*MODE_AVAIL_WRITER)
        ok = all(r[2] for r in rows)
        store = rows[1]
        kva, ova = store[3][0], store[3][1]
        self.mode_avail = (kva, ova) if ok else None
        self.row(ok, 'g_Game_EngineModeAvail writer (0x368-byte object, memset 26 x 0x20 at +0x28)',
                 f'{MODE_AVAIL_WRITER[0]:#x} -> {fr["erva"] + B:#x} ({fr["n"]} instructions, {diff[0]} differ); '
                 f'store {store[0]:#x} -> {store[1]:#x}: global {kva:#x} -> {ova:#x} (RVA {ova - B:#x})')
        (fr, diff), rows = self.aligned(*MODE_AVAIL_READER)
        same = all(r[2] for r in rows)
        ld = rows[0][3]
        call = rows[-1][3]
        setmode = self.bound.get('Car_FuelEcu_SetMode_Thunk', (None, None))[1]
        ok = same and ld[1] == ova and call[3] == setmode
        self.row(ok, 'fuel mix availability test before Car_FuelEcu_SetMode_Thunk (ecu +0x128, obj +0x28 + car*0x20 + mix*4)',
                 f'{MODE_AVAIL_READER[0]:#x} -> {fr["erva"] + B:#x} ({fr["n"]} instructions, {diff[0]} differ); '
                 f'{len(rows)} instructions aligned and identical: {same}; global {ld[1] and hex(ld[1])}, '
                 f'call {call[3] and hex(call[3])} = binding {setmode and hex(setmode)}')


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--kb', required=True, help='the KB build F1_25.exe')
    ap.add_argument('--other', required=True, help='the other build: its F1_25.exe, or a flat image dump')
    ap.add_argument('--build', required=True, help='the other build\'s overlay name in re/db/builds')
    ap.add_argument('--md', help='write the table as Markdown')
    a = ap.parse_args(argv)
    c = Check(bv.Image(a.kb), bv.Image(a.other))
    c.opath = a.other
    c.evidence()
    c.readers()
    c.bindings(a.build)
    c.sites()
    c.globals()
    tag = {True: 'ok    ', False: 'FAIL  ', None: 'review'}
    for ok, what, detail in c.rows:
        print(f'{tag[ok]} {what}: {detail}')
    print(f'{sum(1 for r in c.rows if r[0])} of {len(c.rows)} checks pass, {c.failed} fail, '
          f'{sum(1 for r in c.rows if r[0] is None)} bodies differ in the instructions listed')
    if c.mode_avail:
        print(f'mode_avail_rva {c.mode_avail[1] - B:#x}')
    if a.md:
        with open(a.md, 'w') as f:
            f.write('| | check | evidence |\n|---|---|---|\n')
            for ok, what, detail in c.rows:
                f.write(f'| {tag[ok].strip()} | {what} | {detail.replace("|", "/")} |\n')
    return 1 if c.failed else 0


if __name__ == '__main__':
    sys.exit(main())

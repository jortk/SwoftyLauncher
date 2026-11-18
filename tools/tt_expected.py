#!/usr/bin/env python3
"""Write the expected sealed car values of every stock F1 25 car and every Swofty season car.

    tools/tt_expected.py --out expected.json
    tools/tt_expected.py --out expected.json --check truth.json --report REPORT.md

Every value comes from files: the stock handling files (.vtf in the car ERPs), the stock game database and the season
addons' own car ERPs and car.profile records. Stock files are read from the launcher baseline when it holds the
original, else from the game folder. Nothing is written into the game folder.
"""
import argparse
import collections
import glob
import json
import os
import struct
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import erp
import gamedb
import vtf

GEARS = ['1st', '2nd', '3rd', '4th', '5th', '6th', '7th', '8th', '9th']
FUEL_KINDS = [('fuel_burn', 'FuelBurnScale'), ('fuel_power', 'FuelPowerScale'), ('fuel_revs', 'FuelRevsScale'),
              ('fuel_temperature', 'TemperatureScale')]
FUEL_MODES = ['Lean', 'Standard', 'Rich', 'Max']

# field -> (section, key, default when the key or section is absent)
SOURCES = collections.OrderedDict([
    ('steering_lock', ('Steering', 'MaxSteeringLock', None)),
    ('clutch_torque', ('GearBox', 'MaxClutchTorque', None)),
    ('turbo_max_boost', ('Turbo', 'MaxBoost', 0.0)),
    ('wing_speed_threshold', ('F1WingDamage', 'SpeedThreshold', None)),
    ('wing_damage_scale', ('F1WingDamage', 'DamageScale', None)),
    ('wing_chassis_damage_multiple', ('F1WingDamage', 'ChassisDamageMultiple', None)),
    ('wing_side_detach_scale', ('F1WingDamage', 'SideDetachScale', None)),
    ('wing_full_detach_scale', ('F1WingDamage', 'FullDetachScale', None)),
    ('susp_speed_threshold', ('F1SuspensionDamage', 'SpeedThreshold', None)),
    ('susp_front_damage_scale', ('F1SuspensionDamage', 'FrontDamageScale', None)),
    ('susp_rear_damage_scale', ('F1SuspensionDamage', 'RearDamageScale', None)),
    ('susp_wheel_detach_scale', ('F1SuspensionDamage', 'WheelDetachScale', None)),
    ('susp_parts_detach_scale', ('F1SuspensionDamage', 'PartsDetachScale', None)),
    ('susp_linear_fracture_limit', ('F1SuspensionDamage', 'LinearFractureLimit', None)),
    ('susp_angular_fracture_limit', ('F1SuspensionDamage', 'AngularFractureLimit', None)),
    ('engine_limiter', ('Engine', 'LimiterRotationRate', None)),
    ('final_drive', ('GearBox', 'FinalDriveRatio', None)),
])
for i, g in enumerate(GEARS):
    SOURCES['gear_ratio_%d' % (i + 1)] = ('GearBox', 'GearRatio' + g, 0.0)
for k, (kind, key) in enumerate(FUEL_KINDS):
    for m, mode in enumerate(FUEL_MODES):
        name = kind + '_lean' if m == 0 else 'field_%d' % (48 + k * 4 + m)
        SOURCES[name] = ('FuelECU', key + mode, None)
SOURCES.update([
    ('aero_slipstream_drag', ('F1Aero', 'm_aeroSlipStreamAirDensityScaleDrag', None)),
    ('aero_slipstream_front_wing', ('F1Aero', 'm_aeroSlipStreamAirDensityScaleFrontWing', None)),
    ('aero_slipstream_rear_wing', ('F1Aero', 'm_aeroSlipStreamAirDensityScaleRearWing', None)),
    ('aero_slipstream_lift', ('F1Aero', 'm_aeroSlipStreamAirDensityScaleLift', None)),
    ('aero_slipstream_car_width', ('F1Aero', 'm_aeroSlipStreamCarWidth', None)),
])
# Values no handling file holds: the core's own levers at their neutral value until a car.profile moves them.
LEVERS = {'aero_lift_scale': 1.0, 'aero_drag_scale': 1.0, 'engine_power_scale': 1.0, 'engine_limiter_offset_rpm': 0.0}
# car.profile key -> sealed field
PROFILE = {
    'lift_scale': 'aero_lift_scale', 'drag_scale': 'aero_drag_scale', 'power_scale': 'engine_power_scale',
    'limiter_offset_rpm': 'engine_limiter_offset_rpm',
    'aero.lift_scale': 'aero_lift_scale', 'aero.drag_scale': 'aero_drag_scale',
    'engine.power_scale': 'engine_power_scale', 'engine.limiter_offset_rpm': 'engine_limiter_offset_rpm',
    'aero.slipstream_drag': 'aero_slipstream_drag', 'aero.slipstream_front_wing': 'aero_slipstream_front_wing',
    'aero.slipstream_rear_wing': 'aero_slipstream_rear_wing', 'aero.slipstream_lift': 'aero_slipstream_lift',
    'aero.slipstream_car_width': 'aero_slipstream_car_width',
    'steering.max_steering_lock': 'steering_lock', 'gearbox.max_clutch_torque': 'clutch_torque',
    'turbo.max_boost': 'turbo_max_boost', 'engine.vtf_limiter_rotation_rate': 'engine_limiter',
    'gearbox.final_drive_ratio': 'final_drive',
}
for i in range(9):
    PROFILE['gearbox.gear_ratio_%d' % (i + 1)] = 'gear_ratio_%d' % (i + 1)
# Sealed fields left out of the table: the setup screen or the session writes them.
LEFT_OUT = ['diff_on_throttle', 'diff_off_throttle', 'brakes_bias', 'brakes_pressure', 'fuel_mass',
            'susp_front_camber', 'susp_front_toe', 'susp_front_anti_roll', 'susp_front_ride_height_offset',
            'susp_rear_camber', 'susp_rear_toe', 'susp_rear_anti_roll', 'susp_rear_ride_height_offset',
            'chassis_ballast_kg']
TIERS = ['very_low', 'low', 'medium', 'high', 'very_high']
# vehicle.handling section -> handling file section, as the launcher's activation/handling.rs maps them.
SECTIONS = {
    'engine': 'Engine', 'gearbox': 'GearBox', 'fuel_tank': 'FuelTank', 'fuel_ecu': 'FuelECU', 'brake_ecu': 'BrakeECU',
    'chassis': 'Chassis', 'steering': 'Steering', 'core_aero': 'CoreAero', 'differential': 'Differential',
    'suspension': 'SuspensionSystem', 'f1_aero': 'F1Aero', 'assists': 'F1Assists', 'tuning': 'F1Tuning',
    'plank': 'F1Plank', 'suspension_damage': 'F1SuspensionDamage', 'wing_damage': 'F1WingDamage',
    'tyre_damage': 'F1TyreDamage', 'body_damage': 'F1BodyDamage', 'turbo': 'Turbo', 'ers': 'EnergyRecoverySystem',
}
NOT_HANDLING = ('vtf', 'extends', 'within')


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def shown(x):
    return float('%.9g' % f32(x))


class Stock:
    def __init__(self, game, baseline):
        self.game = game
        self.files, self.objects, self.exe = {}, None, None
        for path in sorted(glob.glob(os.path.join(baseline, '*', 'baseline.json'))) if baseline else []:
            doc = json.load(open(path))
            if doc.get('stock_checked') or not self.files:
                self.files, self.exe = doc['files'], doc['exe_sha256']
                self.objects = os.path.join(baseline, 'objects')
        self.from_game = []

    def path(self, rel):
        rec = self.files.get(rel)
        if rec:
            obj = os.path.join(self.objects, rec['sha256'][:2], rec['sha256'])
            if os.path.exists(obj):
                return obj
        full = os.path.join(self.game, rel)
        if os.path.exists(full):
            self.from_game.append(rel)
            return full
        return None


def read_vtfs(path):
    meta, res = erp.parse(path)
    out = {}
    with open(path, 'rb') as f:
        for name, rtype, _unk, frags, _h in res:
            if rtype != 'VTF' or not frags:
                continue
            _fn, off, _size, _flags, _comp, packed = frags[0]
            f.seek(meta['data_off'] + off)
            raw = f.read(packed)
            r = subprocess.run(['zstd', '-dc'], input=raw, capture_output=True)
            data = r.stdout if r.returncode == 0 else raw
            out[name.rsplit('/', 1)[-1]] = vtf.parse(data)
    return out


def apply_op(old, node):
    x = float(node.attr(b'value'))
    op = node.attr(b'op')
    if old is None or op in (None, b'='):
        return x
    if op == b'*':
        return f32(old) * f32(x)
    if op == b'+':
        return f32(old) + f32(x)
    if op == b'-':
        return f32(old) - f32(x)
    if op == b'/':
        return f32(old) / f32(x)
    raise ValueError('upgrade op %r' % op)


def read_fields(root, block=None, base=None):
    """The sealed fields of a handling file, or of `base` after the named upgrade block."""
    scope = root
    if block is not None:
        found = [u for u in root.find(b'Upgrade') if u.attr(b'name') == block.encode()]
        if not found:
            return dict(base)
        scope = found[0]
    out = dict(base) if base else {}
    for field, (section, key, default) in SOURCES.items():
        sec = scope.find(section.encode())
        node = sec[0].find(key.encode()) if sec else []
        if node:
            out[field] = apply_op(out.get(field) if block else None, node[0])
        elif block is None:
            if default is None:
                raise KeyError('%s/%s' % (section, key))
            out[field] = default
    if block is None:
        out.update(LEVERS)
    # Car_Read_F1SuspensionDamage raises PartsDetachScale to 1.5 x WheelDetachScale.
    out['susp_parts_detach_scale'] = max(f32(out['susp_parts_detach_scale']),
                                         f32(f32(1.5) * f32(out['susp_wheel_detach_scale'])))
    return out


def apply_profile(values, profile):
    out = dict(values)
    for key, v in profile.items():
        field = PROFILE.get(key)
        if field is None or field not in out:
            continue
        if isinstance(v, dict):
            if 'scale' in v:
                out[field] = f32(f32(out[field]) * f32(v['scale']))
            elif 'add' in v:
                out[field] = f32(f32(out[field]) + f32(v['add']))
            elif 'set' in v:
                out[field] = v['set']
        else:
            out[field] = v
    return out


def norm(key):
    return key.replace('_', '').lower()


def apply_record(values, record):
    """The sealed fields after the build applies a vehicle.handling record to the handling file."""
    out = dict(values)
    for snake, fields in (record or {}).items():
        section = SECTIONS.get(snake)
        if section is None or not isinstance(fields, dict):
            continue
        for key, v in fields.items():
            if snake == 'gearbox' and key == 'ratios' and isinstance(v, list):
                for i in range(len(GEARS)):
                    out['gear_ratio_%d' % (i + 1)] = float(v[i]) if i < len(v) else 0.0
                continue
            if isinstance(v, bool) or not isinstance(v, (int, float)):
                continue
            for field, (sec, name, _default) in SOURCES.items():
                if sec == section and norm(name) == norm(key):
                    out[field] = float(v)
    return out


def resolve(records, rid, addon, seen=()):
    rec = records[rid]
    parent = rec.get('extends')
    if not parent:
        return rec
    pid = parent.split('/', 1)[1] if parent.startswith(addon + '/') else parent
    if pid not in records or pid in seen:
        return rec
    return deep_merge(resolve(records, pid, addon, seen + (rid,)), rec)


def deep_merge(base, over):
    out = dict(base)
    for k, v in over.items():
        if isinstance(v, dict) and isinstance(out.get(k), dict):
            out[k] = deep_merge(out[k], v)
        else:
            out[k] = v
    out.pop('extends', None)
    return out


def handling_package(record):
    for res in (record.get('vtf') or {}).get('resources') or []:
        if res.startswith('eaid://'):
            return res[len('eaid://'):].split('/', 1)[0]
    return None


def agree(values, total):
    if len(values) * 2 < total:
        return None
    if all(isinstance(v, dict) for v in values):
        out = {}
        for key in sorted({k for v in values for k in v}):
            got = agree([v[key] for v in values if key in v], total)
            if got is not None:
                out[key] = got
        return out or None
    counts = []
    for v in values:
        text = json.dumps(v, sort_keys=True)
        hit = next((c for c in counts if c[0] == text), None)
        if hit:
            hit[1] += 1
        else:
            counts.append([text, 1, v])
    best = max(c[1] for c in counts)
    return next(c[2] for c in counts if c[1] == best)


def consensus(records):
    """The season's equal performance base car: each value most of the package's handling records share, as the
    launcher's activation/handling.rs builds it."""
    keys = sorted({k for r in records for k in r if k not in NOT_HANDLING})
    out = {}
    for key in keys:
        got = agree([r[key] for r in records if key in r], len(records))
        if got is not None:
            out[key] = got
    return out


def merge(variants):
    out = {}
    for field in variants[0]:
        vals = []
        for v in variants:
            s = shown(v[field])
            if s not in vals:
                vals.append(s)
        out[field] = vals
    return out


def car_entry(content, version, stat, team, roots, tiers, profile=None, drop=(), records=None):
    """roots: the handling files a car may be built from (its own, and its equal performance base), with the
    vehicle.handling record the build applies to each in `records`."""
    records = records or [None] * len(roots)
    base = [apply_record(read_fields(r), rec) for r, rec in zip(roots, records)]
    fin = (lambda v: apply_profile(v, profile)) if profile else (lambda v: v)
    values = merge([fin(b) for b in base])
    for f in drop:
        values.pop(f)
    tracks = {}
    for track, tier in sorted(tiers.items()):
        per = merge([fin(read_fields(r, tier, b)) for r, b in zip(roots, base)])
        diff = {f: v for f, v in per.items() if f in values and v != values[f]}
        if diff:
            tracks[track] = diff
    entry = collections.OrderedDict([('content', content), ('version', version), ('vehicle_stat', stat), ('team', team),
                                     ('values', values), ('tracks', tracks)])
    if drop:
        entry['left_out'] = list(drop)
    return entry


class Database:
    def __init__(self, path):
        self.db = {r.key: r.value.decode('utf-8', 'replace') for r in gamedb.read(open(path, 'rb').read())}

    def get(self, key):
        return self.db.get(gamedb.fnv1a64(key))

    def rows(self, table):
        i = 0
        while self.get('%s_%d._metadata_' % (table, i)) is not None:
            yield '%s_%d' % (table, i)
            i += 1


def stock_cars(stock, db, packages):
    tiers = {}
    for row in db.rows('tracks.Track'):
        level = db.get(row + '.downForceLevel')
        if level in TIERS:
            tiers[db.get(row + '._metadata_')] = level
    resource = {}
    for row in db.rows('entrants.vehicles.vehicle_packages.vehicle_package'):
        resource[db.get(row + '._metadata_')] = db.get(row + '.resource_path')
    vehicles = {}
    for row in db.rows('entrants.vehicles.vehicle'):
        vehicles[db.get(row + '._metadata_')] = row
    team_of = {}
    lists = list(db.rows('entrants.entrant_lists.entrant_list')) + ['entrants.entrant_lists.entrants']
    for lst in lists:
        for row in db.rows(lst + '.entrant'):
            v, team = db.get(row + '.vehicle'), db.get(row + '.team')
            if v and team:
                team_of.setdefault(v, team)
    cache = {}

    def vtfs(rel):
        if rel not in cache:
            p = stock.path(rel)
            cache[rel] = read_vtfs(p) if p else {}
        return cache[rel]

    def erp_of(name):
        row = vehicles[name]
        pattern = resource.get(db.get(row + '.vehicle_package')) or ''
        asset = db.get(row + '.asset_filename')
        return asset, '2025_asset_groups/' + pattern.replace('[FILENAME]', asset) + '.erp', pattern

    cars, problems = [], []
    for name, row in vehicles.items():
        package = db.get(row + '.vehicle_package')
        if package not in packages:
            continue
        stat = int(db.get(row + '.stat_id'))
        asset, rel, pattern = erp_of(name)
        own = vtfs(rel).get(asset + '.vtf')
        if own is None:
            rel = '2025_asset_groups/' + pattern.split('[FILENAME]')[0] + 'common.erp'
            own = vtfs(rel).get(asset + '.vtf')
        if own is None:
            problems.append('%s (stat %d): no %s.vtf in %s' % (name, stat, asset, rel))
            continue
        roots, sources = [own], [rel + ':' + asset + '.vtf']
        equal = db.get(row + '.equal_performance_vehicle')
        if equal and equal in vehicles and equal != name:
            easset, erel, _ = erp_of(equal)
            common = '2025_asset_groups/' + pattern.split('[FILENAME]')[0] + 'common.erp'
            for cand_rel, cand_name in ((rel, '%s_%s.vtf' % (easset, asset)), (erel, easset + '.vtf'),
                                        (common, easset + '.vtf')):
                base = vtfs(cand_rel).get(cand_name)
                if base is not None:
                    roots.append(base)
                    sources.append(cand_rel + ':' + cand_name)
                    break
            else:
                problems.append('%s (stat %d): equal performance base %s not found' % (name, stat, equal))
        entry = car_entry('game/f1_2025', stock.exe, stat, team_of.get(name, name), roots, tiers)
        entry['vehicle'] = name
        entry['sources'] = sources
        cars.append(entry)
    cars.sort(key=lambda c: c['vehicle_stat'])
    return cars, tiers, problems


def season_cars(addon_dir, stock, db, tiers):
    manifest = json.load(open(os.path.join(addon_dir, 'swofty.json')))
    reg_path = os.path.join(addon_dir, 'registry', 'content.json')
    if not os.path.exists(reg_path):
        return [], []
    reg = json.load(open(reg_path))
    addon = manifest['id']
    cars, problems = [], []
    raw = reg.get('vehicle.handling', {})
    handling = {rid: resolve(raw, rid, addon) for rid in raw}
    by_erp, by_package = {}, collections.defaultdict(list)
    for rid, rec in handling.items():
        by_package[handling_package(rec)].append(rec)
        erp_rel = (rec.get('vtf') or {}).get('erp')
        if erp_rel:
            by_erp.setdefault(erp_rel, rec)
    put, groups = {}, []
    for rel in manifest.get('operations', {}).get('assets', []):
        for op in json.load(open(os.path.join(addon_dir, rel))).get('ops', []):
            if op.get('op') == 'file.put':
                put[op['target']] = os.path.join(addon_dir, op['source'])
            elif op.get('op') == 'group.register':
                groups.append(op.get('erps', []))
    cache = {}

    def group_vtf(target, name):
        for erps in groups:
            if target in erps:
                for rel in erps:
                    if rel not in cache:
                        p = put.get(rel) or stock.path(rel)
                        cache[rel] = read_vtfs(p) if p and os.path.exists(p) else {}
                    if name in cache[rel]:
                        return cache[rel][name]
        return None
    for package, rec in reg.get('content.package', {}).items():
        content = '%s/%s' % (addon, package)
        profile = {}
        for pid, prof in reg.get('car.profile', {}).items():
            within = prof.get('within') or []
            if any(w.get('content') == content for w in within) and all(set(w) == {'content'} for w in within):
                profile.update({k: v for k, v in prof.items() if k != 'within'})
            elif any(w.get('content') == content for w in within):
                problems.append('%s: car.profile %s has a selector besides content, not applied' % (content, pid))
        for team in rec.get('teams', []):
            vkey = team['vehicle'].split('/')[-1]
            vrec = reg.get('vehicle', {}).get(vkey, {})
            # The car is built from the archive of its `ai.asset_filename`, the car whose pace it takes.
            vkey = (vrec.get('ai') or {}).get('asset_filename') or vkey
            label = team['team'].split('/')[-1]
            path = os.path.join(addon_dir, 'cars', 'teams', vkey, 'wep', vkey + '.erp')
            if not os.path.exists(path):
                problems.append('%s %s: no car archive %s' % (content, label, os.path.relpath(path, addon_dir)))
                continue
            files = read_vtfs(path)
            own = files.get(vkey + '.vtf') or (list(files.values())[0] if len(files) == 1 else None)
            if own is None:
                problems.append('%s %s: no handling file in its car archive' % (content, label))
                continue
            own_rec = by_erp.get(os.path.relpath(path, addon_dir).replace(os.sep, '/'))
            roots, records = [own], [own_rec]
            target = [t for t, src in put.items() if os.path.abspath(src) == os.path.abspath(path)]
            base = files.get('fom_car_%s.vtf' % vkey) or (group_vtf(target[0], 'fom_car.vtf') if target else None)
            if base is not None:
                roots.append(base)
                # A season with handling records drives the consensus of its package's records as its equal
                # performance base car; the stock base is never driven then.
                package = handling_package(own_rec) if own_rec else None
                records.append(consensus(by_package[package]) if package else None)
            else:
                problems.append('%s %s: no equal performance base found' % (content, label))
            drop = [PROFILE[k] for k in ('lift_scale', 'drag_scale') if profile.get(k, 1.0) != 1.0]
            cars.append(car_entry(content, manifest['version'], None, label, roots, tiers, profile, drop=drop,
                                  records=records))
    return cars, problems


def close(a, b):
    return abs(a - b) <= 1e-6 * max(1.0, abs(a), abs(b))


def check(table, truth, report_path):
    by_stat, by_team = {}, {}
    for c in table['cars']:
        if c['vehicle_stat'] is not None:
            by_stat[(c['content'], c['vehicle_stat'])] = c
        else:
            by_team[(c['content'], c['version'], c['team'])] = c
    stats = collections.OrderedDict()
    fields_bad = collections.defaultdict(collections.Counter)
    lines = []
    for x in truth:
        content = x['content']
        car = by_stat.get((content, x['vehicle_stat'])) if content == 'game/f1_2025' else \
            by_team.get((content, x['version'], x['team']))
        key = '%s %s' % (content, x['version'] if content != 'game/f1_2025' else '')
        s = stats.setdefault(key.strip(), collections.Counter())
        s['keys'] += 1
        if car is None:
            s['no_table_entry'] += 1
            continue
        s['covered'] += 1
        ordered = sorted(x['sets'], key=lambda z: -z['drivers'])
        for n, st in enumerate(ordered):
            bad = {}
            for f, allowed in car['values'].items():
                allowed = car['tracks'].get(x['track'], {}).get(f, allowed)
                if f in st['values'] and not any(close(st['values'][f], a) for a in allowed):
                    bad[f] = (st['values'][f], allowed)
            s['laps'] += st['drivers']
            if n == 0:
                s['majority_agree' if not bad else 'majority_differ'] += 1
            if bad:
                s['laps_differ'] += st['drivers']
                sig = tuple(sorted(bad))
                fields_bad[key.strip()][sig] += st['drivers']
                if n == 0:
                    lines.append((key.strip(), x['vehicle_stat'], x['team'], x['track'], st['drivers'], bad))
    out = ['| content | keys | with a table entry | majority set agrees | majority set differs | no table entry |',
           '|---|---|---|---|---|---|']
    for k, s in stats.items():
        out.append('| %s | %d | %d | %d | %d | %d |' % (k, s['keys'], s['covered'], s['majority_agree'],
                                                         s['majority_differ'], s['no_table_entry']))
    detail = []
    for k, counter in fields_bad.items():
        for sig, n in counter.most_common(12):
            detail.append('- %s: %d driver(s) differ on %s' % (k, n, ', '.join(sig)))
    majority = []
    for k, stat, team, track, drivers, bad in lines[:400]:
        majority.append('- %s stat %s %s at %s (%d driver(s)): %s' % (
            k, stat, team, track, drivers,
            '; '.join('%s %.9g not in %s' % (f, v, a) for f, (v, a) in sorted(bad.items())[:6])))
    if report_path:
        with open(report_path, 'w') as f:
            f.write('\n'.join(out) + '\n\n## Differing sets by field group\n\n' + '\n'.join(detail) +
                    '\n\n## Majority sets that differ\n\n' + '\n'.join(majority) + '\n')
    return out, detail, majority


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--game', default='/mnt/gaming/SteamLibrary/steamapps/common/F1 25')
    ap.add_argument('--data', default=os.path.expanduser('~/.local/share/swofty'), help='launcher data folder')
    ap.add_argument('--addons', help='folder of installed addons (default: <data>/addons)')
    ap.add_argument('--addon-prefix', action='append', default=None,
                    help='addon id prefix to include (default: swofty.swofty-seasons-)')
    ap.add_argument('--packages', default='f1_2025,f1_2026', help='stock vehicle packages')
    ap.add_argument('--out', required=True)
    ap.add_argument('--check', help='truth.json of accepted laps to compare against')
    ap.add_argument('--report', help='where --check writes its tables')
    args = ap.parse_args()
    stock = Stock(args.game, os.path.join(args.data, 'baseline'))
    db_path = stock.path('game_package/gamedatabases/gamedatabases.2025.bin')
    if not db_path:
        sys.exit('no game database under %s' % args.game)
    db = Database(db_path)
    cars, tiers, problems = stock_cars(stock, db, set(args.packages.split(',')))
    prefixes = args.addon_prefix or ['swofty.swofty-seasons-']
    for d in sorted(glob.glob(os.path.join(args.addons or os.path.join(args.data, 'addons'), '*'))):
        if any(os.path.basename(d).startswith(p) for p in prefixes) and os.path.exists(os.path.join(d, 'swofty.json')):
            more, bad = season_cars(d, stock, db, tiers)
            cars += more
            problems += bad
    table = collections.OrderedDict([('version', 1), ('left_out', LEFT_OUT), ('cars', cars)])
    with open(args.out, 'w') as f:
        json.dump(table, f, separators=(',', ':'))
    print('%d cars -> %s' % (len(cars), args.out), file=sys.stderr)
    for p in problems:
        print('problem: ' + p, file=sys.stderr)
    if stock.from_game:
        print('read from the game folder (no baseline original): %d file(s)' % len(set(stock.from_game)), file=sys.stderr)
    if args.check:
        out, detail, majority = check(table, json.load(open(args.check)), args.report)
        print('\n'.join(out))
        print('\n'.join(detail[:60]))


if __name__ == '__main__':
    main()

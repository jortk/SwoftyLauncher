#!/usr/bin/env python3
"""Write the expected integrity-seal `db` hashes of the stock F1 25 game database and of every Swofty season.

    tools/tt_expected_db.py --out expected-db.json
    tools/tt_expected_db.py --out expected-db.json --check truth.json --report REPORT.md

The seal hashes rows of the one game database the game loaded, so a hash follows every enabled addon that edits
the row, whatever content the lap was driven in. The table holds:

    stock     name -> hash of the stock database (the game's own content expects exactly these)
    seasons   per season content and version: `db`, the names that season's own database operations change
    any       name -> every hash a row takes with some Swofty season enabled (stock first), for the rows a
              season other than the lap's own may have changed

A name no table lists (a row a track or car addon adds) has no expected hash. The stock database is read from
the launcher baseline when it holds the original, else from the game folder. Nothing is written into the game folder.
"""
import argparse
import collections
import glob
import hashlib
import json
import os
import re
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gamedb
from tt_expected import Stock

DB_FILE = 'game_package/gamedatabases/gamedatabases.2025.bin'
# integrity/src/db_keys.hpp
KEYS = ['difficulty.mediumTractionControl', 'collision_assistance.collisionMassModifier_human',
        'vehicle_helpers.critically_low_fuel_expected_speed_scalar_easy',
        'vehicle_helpers.critically_low_fuel_expected_speed_scalar_hard']
TABLES = [
    ('tracks', 'Track', None,
     ['ERSSettings.deployFactor', 'ERSSettings.harvestFactor', 'ERSSettings.hotlapFactor', 'fuel_burn_per_lap',
      'tyre_wear', 'Tyres_0.tyre_grip_modifier']),
    ('entrants.vehicles.vehicle_packages', 'vehicle_package', None,
     ['resource_path', 'resource_path_specialisation', 'tyre_compounds_asset_group',
      'is_equal_performance_vehicle_package', 'is_26_aero_regulations']),
    ('car_setup.sliders', 'slider', None, ['min', 'max', 'steps', 'reverse_direction']),
    ('tyres.physical_compounds', 'PhysicalCompound', None, ['physics_code', 'is_slick', 'is_full_wet']),
    ('tyres.tyre_wear_scales', 'TrackTyreWearScaleSet', 'track_tyre_wear_scale',
     ['Softer', 'Medium', 'Harder', 'Intermediate', 'Wet']),
]
ROW_CAP, ENTRY_CAP = 96, 512
SELECT = re.compile(r'^(?P<table>[^\[]+)\.(?P<stem>[^.\[]+)\[_metadata_=(?P<label>[^\]]+)\]$')


def sha(text):
    return hashlib.sha256(text.encode('utf-8')).hexdigest()


def db_text(rec):
    """The text the seal hashes for a record: the core reads int, float and bool records as numbers, string,
    enum and id records as tag 2 text, and no other record type."""
    typ, value = rec
    s = value.decode('utf-8', 'replace')
    if typ == 0:
        return '0:%d' % int(s)
    if typ == 5:
        return '5:%d' % (s.lower() == 'true')
    if typ == 1:
        return '1:%.9g' % struct.unpack('<f', struct.pack('<f', float(s)))[0]
    if typ in (2, 3, 4):
        return '2:' + s
    return None


class Records:
    def __init__(self, path=None, recs=None):
        self.recs = recs if recs is not None else {r.key: (r.type, r.value) for r in
                                                   gamedb.read(open(path, 'rb').read())}

    def copy(self):
        return Records(recs=dict(self.recs))

    def get(self, key):
        return self.recs.get(gamedb.fnv1a64(key))

    def count(self, parent, stem):
        rec = self.get('%s._%s_instancecount_' % (parent, stem))
        if not rec or rec[0] != 0 or int(rec[1]) < 0:
            return 0
        return int(rec[1])

    def label(self, row):
        rec = self.get(row + '._metadata_')
        if not rec or db_text(rec) is None:
            return None
        s = rec[1].decode('utf-8', 'replace')
        return s if 0 < len(rec[1]) <= 96 else None

    def find(self, table, stem, label):
        for i in range(self.count(table, stem)):
            row = '%s.%s_%d' % (table, stem, i)
            if self.label(row) == label:
                return row
        return None

    def set(self, key, value):
        old = self.get(key)
        if isinstance(value, bool):
            value = 'True' if value else 'False'
        self.recs[gamedb.fnv1a64(key)] = (old[0] if old else 2, str(value).encode('utf-8'))


def scan(db):
    """integrity/src/db_scan.hpp db_scan over a database file's records."""
    out = collections.OrderedDict()

    def row_hash(row, fields):
        text = ''
        for field in fields:
            rec = db.get(row + '.' + field)
            text += '%s=%s;' % (field, (db_text(rec) if rec else None) or '-')
        return sha(text)

    for key in KEYS:
        rec = db.get(key)
        if rec and db_text(rec) is not None:
            out[key] = sha(db_text(rec))
    for parent, stem, child, fields in TABLES:
        for i in range(min(db.count(parent, stem), ROW_CAP)):
            if len(out) >= ENTRY_CAP:
                break
            row = '%s.%s_%d' % (parent, stem, i)
            label = db.label(row)
            if not label:
                continue
            if not child:
                out['%s:%s' % (parent, label)] = row_hash(row, fields)
                continue
            for j in range(min(db.count(row, child), ROW_CAP)):
                if len(out) >= ENTRY_CAP:
                    break
                sub = '%s.%s_%d' % (row, child, j)
                sub_label = db.label(sub)
                if sub_label:
                    out['%s:%s:%s' % (parent, label, sub_label)] = row_hash(sub, fields)
    return out


def hashed(target):
    return any(target == k for k in KEYS) or any(target == t[0] or target.startswith(t[0] + '.') for t in TABLES)


def season(addon_dir, stock_db):
    """The hashes a season's own database operations change, by content."""
    manifest = json.load(open(os.path.join(addon_dir, 'swofty.json')))
    db, problems = stock_db.copy(), []
    for rel in manifest.get('operations', {}).get('database', []):
        for op in json.load(open(os.path.join(addon_dir, rel))).get('ops', []):
            target = op.get('select') or op.get('table') or op.get('key') or ''
            if not hashed(target.split('[')[0]):
                continue
            m = SELECT.match(target)
            if op.get('op') != 'set' or not m:
                problems.append('%s: %s on %s is not modelled' % (manifest['id'], op.get('op'), target))
                continue
            row = db.find(m.group('table'), m.group('stem'), m.group('label'))
            if row is None:
                problems.append('%s: no row %s' % (manifest['id'], target))
                continue
            db.set('%s.%s' % (row, op['field']), op['value'])
    reg_path = os.path.join(addon_dir, 'registry', 'content.json')
    packages = json.load(open(reg_path)).get('content.package', {}) if os.path.exists(reg_path) else {}
    stock, mine = scan(stock_db), scan(db)
    if set(stock) != set(mine):
        problems.append('%s: its operations add or remove hashed rows' % manifest['id'])
    changed = collections.OrderedDict((n, h) for n, h in mine.items() if stock.get(n) != h)
    return [collections.OrderedDict([('content', '%s/%s' % (manifest['id'], p)), ('versions', [manifest['version']]),
                                     ('db', changed)]) for p in packages], problems


def allowed(table, content, version):
    """name -> the hashes a lap of `content` at `version` may carry, or None when no table covers it."""
    out = {n: [h] for n, h in table['stock'].items()}
    for n, hs in table['any'].items():
        out[n] = list(hs)
    if content == table['game']['content']:
        return out if version in table['game']['versions'] else None
    for s in table['seasons']:
        if s['content'] == content and version in s['versions']:
            for n, h in s['db'].items():
                out[n] = [h]
            return out
    return None


def check(table, truth, report_path):
    """truth: {"laps": {key: n}, "db": {key: {name: {hash: laps}}}}, key = content|version|build|status."""
    groups = collections.OrderedDict()
    for key in sorted(truth['laps']):
        content, version, build, _status = key.split('|')
        group = '%s %s' % (content, build if content == table['game']['content'] else version)
        g = groups.setdefault(group, {'laps': 0, 'names': collections.defaultdict(collections.Counter),
                                      'content': content, 'version': version, 'build': build})
        g['laps'] += truth['laps'][key]
        for name, tally in truth['db'][key].items():
            g['names'][name].update(tally)
    lines = ['| content and version | laps | names sealed | in the table | majority hash allowed | majority differs '
             '| laps with a hash not allowed |', '|---|---|---|---|---|---|---|']
    detail = []
    for group, g in groups.items():
        version = g['version']
        if g['content'] == table['game']['content']:
            version = [v for v in table['game']['versions'] if v.startswith(g['build'])]
            version = version[0] if version else g['build']
        want = allowed(table, g['content'], version)
        if want is None:
            lines.append('| %s | %d | %d | no table | | | |' % (group, g['laps'], len(g['names'])))
            continue
        covered = agree = differ = 0
        worst = 0
        for name, tally in sorted(g['names'].items()):
            if name not in want:
                continue
            covered += 1
            top = tally.most_common(1)[0][0]
            if top in want[name]:
                agree += 1
            else:
                differ += 1
                detail.append('- %s: majority of `%s` is %s (%d of %d laps), table allows %s' % (
                    group, name, top[:12], tally[top], sum(tally.values()), ', '.join(h[:12] for h in want[name])))
            off = sum(n for h, n in tally.items() if h not in want[name])
            if off:
                worst = max(worst, off)
                detail.append('- %s: %d of %d laps carry another hash of `%s` (%s)' % (
                    group, off, sum(tally.values()), name,
                    ', '.join('%s x%d' % (h[:12], n) for h, n in tally.most_common() if h not in want[name])))
        lines.append('| %s | %d | %d | %d | %d | %d | at most %d |' % (group, g['laps'], len(g['names']), covered,
                                                                       agree, differ, worst))
    if report_path:
        with open(report_path, 'w') as f:
            f.write('\n'.join(lines) + '\n\n## Hashes outside the table\n\n' + '\n'.join(detail) + '\n')
    return lines, detail


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--game', default='/mnt/gaming/SteamLibrary/steamapps/common/F1 25')
    ap.add_argument('--data', default=os.path.expanduser('~/.local/share/swofty'), help='launcher data folder')
    ap.add_argument('--addons', help='folder of installed addons (default: <data>/addons)')
    ap.add_argument('--addon-prefix', action='append', default=None,
                    help='addon id prefix to include (default: swofty.swofty-seasons-)')
    ap.add_argument('--out', required=True)
    ap.add_argument('--check', help='tallies of sealed laps to compare against')
    ap.add_argument('--report', help='where --check writes its tables')
    args = ap.parse_args()
    baseline = os.path.join(args.data, 'baseline')
    stock = Stock(args.game, baseline)
    path = stock.path(DB_FILE)
    if not path:
        sys.exit('no game database under %s' % args.game)
    file_sha = hashlib.sha256(open(path, 'rb').read()).hexdigest()
    builds = []
    for doc_path in sorted(glob.glob(os.path.join(baseline, '*', 'baseline.json'))):
        doc = json.load(open(doc_path))
        if doc.get('files', {}).get(DB_FILE, {}).get('sha256') == file_sha and doc['exe_sha256'] not in builds:
            builds.append(doc['exe_sha256'])
    stock_db = Records(path)
    stock_hashes = scan(stock_db)
    seasons, problems = [], []
    prefixes = args.addon_prefix or ['swofty.swofty-seasons-']
    for d in sorted(glob.glob(os.path.join(args.addons or os.path.join(args.data, 'addons'), '*'))):
        if any(os.path.basename(d).startswith(p) for p in prefixes) and os.path.exists(os.path.join(d, 'swofty.json')):
            more, bad = season(d, stock_db)
            seasons += [s for s in more if s['db']]
            problems += bad
    any_of = collections.OrderedDict()
    for s in seasons:
        for name, h in s['db'].items():
            hs = any_of.setdefault(name, [stock_hashes[name]])
            if h not in hs:
                hs.append(h)
    table = collections.OrderedDict([
        ('version', 1),
        ('game', collections.OrderedDict([('content', 'game/f1_2025'), ('versions', builds or [stock.exe]),
                                          ('database_sha256', file_sha)])),
        ('stock', stock_hashes), ('any', any_of), ('seasons', seasons)])
    with open(args.out, 'w') as f:
        json.dump(table, f, separators=(',', ':'))
    print('%d stock names, %d seasons -> %s' % (len(stock_hashes), len(seasons), args.out), file=sys.stderr)
    for p in problems:
        print('problem: ' + p, file=sys.stderr)
    if stock.from_game:
        print('read from the game folder (no baseline original): %d file(s)' % len(set(stock.from_game)), file=sys.stderr)
    if args.check:
        lines, detail = check(table, json.load(open(args.check)), args.report)
        print('\n'.join(lines))
        print('%d detail line(s)' % len(detail))


if __name__ == '__main__':
    main()

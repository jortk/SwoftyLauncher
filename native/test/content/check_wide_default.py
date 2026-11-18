#!/usr/bin/env python3
"""A content.package of more than 20 seats that names no entrant_list_from or standings_from copies f1_2025 and
f1_teams_2024 like any other package, not the F1 2026 rows: those carry season_2026 content the player may not own.
Runs the content lowering on Classic Seasons 2016 (22 cars) with its database overrides removed."""

import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
spec = importlib.util.spec_from_file_location("check_lowering", HERE / "check_lowering.py")
lowering = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lowering)

ADDON = "swofty.classic-seasons-2016"
LISTS = "entrants.entrant_lists.entrant_list"
STANDINGS = "entrants.standings.standing"


def lower(tool, overrides):
    folder = HERE / "classic-seasons" / "classic-seasons-2016"
    records = json.loads((folder / "db" / "records.json").read_text())
    content = json.loads((folder / "registry" / "content.json").read_text())
    for package in content["content.package"].values():
        package["database"].pop("entrant_list_from", None)
        package["database"].pop("standings_from", None)
        package["database"].update(overrides)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        (tmp / "stock.bin").write_bytes(lowering.fake_stock(records, content))
        (tmp / "content.json").write_text(json.dumps(content))
        r = subprocess.run([tool, str(ROOT / "packages/sdk/std/registries"), str(tmp / "stock.bin"), str(tmp / "ops.json"),
                            ADDON, str(tmp / "content.json")], capture_output=True, text=True)
        if r.returncode:
            sys.exit(f"lower_tool failed ({r.returncode})\n{r.stdout}{r.stderr}")
        return json.loads((tmp / "ops.json").read_text())["ops"]


def sources(ops):
    lists = {op["from"] for op in ops if op.get("op") == "add-row" and op.get("table") == "entrants.entrant_lists"}
    standings = {op["from"] for op in ops if op.get("op") == "add-row" and op.get("table") == "entrants.standings"}
    return lists, standings


def main():
    tool = sys.argv[1]
    bad = 0
    lists, standings = sources(lower(tool, {}))
    ok = len(lists) == 1 and "_metadata_=f1_2025]" in next(iter(lists)) and \
        len(standings) == 1 and "_metadata_=f1_teams_2024]" in next(iter(standings))
    print(f"22 seats, no overrides: lists {sorted(lists)}, standings {sorted(standings)}: {'ok' if ok else 'DIFF'}")
    bad += not ok
    lists, standings = sources(lower(tool, {"entrant_list_from": "f1_2026", "standings_from": "f1_teams_2026"}))
    ok = len(lists) == 1 and "_metadata_=f1_2026]" in next(iter(lists)) and \
        len(standings) == 1 and "_metadata_=f1_teams_2026]" in next(iter(standings))
    print(f"22 seats, f1_2026 named: lists {sorted(lists)}, standings {sorted(standings)}: {'ok' if ok else 'DIFF'}")
    bad += not ok
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())

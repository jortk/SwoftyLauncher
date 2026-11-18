"""Exercise the generic chart example through its JSON-lines addon protocol."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile


def call_helper(executable, capabilities, directory, version="1.9.52"):
    hello = {"launcher": version}
    if capabilities is not None:
        hello["capabilities"] = capabilities
    requests = [
        {"id": 1, "method": "hello", "params": hello},
        {"id": 2, "method": "view", "params": {"section": "charts"}},
        {"id": 3, "method": "event", "params": {
            "section": "charts", "event": {"click": {"id": "change-data"}}}},
        {"id": 4, "method": "view", "params": {"section": "charts"}},
        {"id": 5, "method": "view", "params": {"section": "charts"}},
        {"id": 6, "method": "tick", "params": {"section": "charts"}},
    ]
    result = subprocess.run(
        [str(executable)], input="".join(json.dumps(r) + "\n" for r in requests),
        text=True, capture_output=True, cwd=directory, timeout=10, check=True,
    )
    replies = [json.loads(line) for line in result.stdout.splitlines()]
    assert [r["id"] for r in replies] == [1, 2, 3, 4, 5, 6]
    assert replies[0]["result"]["api"] == 1
    assert replies[2]["result"] == "reload", "event must return an Effect, not a View"
    assert replies[5]["result"] is None, "tick results are null"
    return [replies[i]["result"]["body"]["blocks"] for i in (1, 3, 4)]


def main():
    executable = Path(sys.argv[1]).resolve()
    if not executable.is_file() and executable.with_suffix(".exe").is_file():
        executable = executable.with_suffix(".exe")
    executable = executable.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="native-chart-protocol-") as temporary:
        before, after, persisted = call_helper(executable, ["view.chart"], temporary)
        original = next(b["chart"] for b in before if "chart" in b)
        changed = next(b["chart"] for b in after if "chart" in b)
        assert original["id"] == changed["id"]
        assert original["series"][0]["points"] != changed["series"][0]["points"]
        assert after == persisted
        assert original["series"][0]["points"][9] is None
        for caps in (None, [], ["future.other"]):
            legacy = call_helper(executable, caps, temporary)
            assert not any("chart" in b for blocks in legacy for b in blocks)
        future = call_helper(executable, ["future.other", "view.chart"], temporary, "99.0.0")
        assert any("chart" in b for blocks in future for b in blocks)
        assert not list(Path(temporary).iterdir()), "Helper created a chart file"
    print("PASS: capability gating, legacy/future hosts, reload effect, persistent view, null tick, gaps, no chart files")


if __name__ == "__main__":
    main()

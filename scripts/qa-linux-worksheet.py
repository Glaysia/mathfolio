"""Replay real pointer/key input in an owned qa-linux-desktop session.

Saves through Ctrl+S, then independently reads the resulting worksheet through
the CLI and checks displayed answers. No application state injection is used.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from PIL import ImageGrab

CASES = [
    (210, 210, "2*3+4=", 10),
    (210, 310, "sin(0)=", 0),
    (210, 410, "(2+3)*4=", 20),
    (210, 510, "-2^2=", -4),
    (210, 610, "1e-3+2=", 2.001),
    (210, 710, "1..3=", {"kind":"range","start":1,"step":1,"end":3,"count":3}),
    (210, 895, "abs(3+4i)=", 5),
    (600, 210, "f(x):=x^2", None),
    (600, 310, "f(3)=", 9),
    (600, 410, "sqrt(9)=", 3),
    (600, 510, "d:=2m", None),
    (600, 610, "d=cm", 200),
    (600, 710, "A:=[1,2;3,4]", None),
    (600, 830, "A[1,0]=", 3),
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("session", type=Path)
    parser.add_argument("cli", type=Path)
    args = parser.parse_args()
    session = json.loads((args.session / "session.json").read_text())
    env = os.environ.copy()
    env.update(session["environment"])

    def xdo(*command):
        subprocess.run(["xdotool", *command], env=env, check=True, timeout=15)

    actions = []
    for x, y, expression, expected in CASES:
        xdo("mousemove", "--sync", str(x), str(y))
        xdo("click", "1")
        xdo("type", "--clearmodifiers", "--delay", "60", "--", expression)
        actions.append(dict(x=x, y=y, expression=expression, expected=expected))
    xdo("key", "--clearmodifiers", "ctrl+s")
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        document = json.loads(Path(session["worksheet"]).read_text())
        if len(document["regions"]) == len(CASES):
            break
        time.sleep(0.1)
    else:
        raise AssertionError("GUI save did not produce all expected regions")
    time.sleep(0.3)
    ImageGrab.grab(xdisplay=session["display"]).save(args.session / "worksheet.png")
    replay = subprocess.run([str(args.cli.resolve()), "sheet-run", session["worksheet"]], text=True, capture_output=True, timeout=15)
    try:
        output = json.loads(replay.stdout)
    except json.JSONDecodeError:
        raise AssertionError(replay.stdout + replay.stderr)
    results = {int(row["region_id"]): row for row in output["rows"]}
    failures = []

    def numeric(value):
        if value is None:
            return None
        if value["kind"] == "scalar":
            q = value["quantity"]
            # num_complex serde serializes a [real, imag] pair.
            if abs(q["value"][1]) > 1e-10:
                return q["value"]
            return q["value"][0]
        if value["kind"] == "matrix":
            return [q["value"][0] for q in value["elements"]]
        if value["kind"] == "range":
            return {"kind":"range","start":value["start"]["value"][0],"step":value["step"]["value"][0],"end":value["end"]["value"][0],"count":value["count"]}
        if value["kind"] == "range_values":
            return [q["value"][0] for q in value["elements"]]
        return value

    for region, (_, _, expression, expected) in zip(document["regions"], CASES):
        row = results.get(region["id"])
        actual = numeric((row.get("display_value") or row.get("value")) if row else None)
        match = row is not None and row.get("error") is None
        if isinstance(expected, (int, float)):
            match &= isinstance(actual, (int, float)) and abs(actual - expected) < 1e-9
        else:
            match &= actual == expected
        if not match:
            failures.append(dict(expression=expression, expected=expected, actual=actual, row=row))
    report = dict(actions=actions, checked=len(CASES), failures=failures, cli_exit=replay.returncode)
    (args.session / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2))
    print(json.dumps(dict(checked=len(CASES), failures=failures), ensure_ascii=False))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

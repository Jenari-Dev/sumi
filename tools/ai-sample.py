#!/usr/bin/env python3
"""Log what AI Visual sees while you play, for tuning its prompts.

Read-only: asks sumid to classify the screen (AiClassifyNow), which switches
nothing. One JSON line per sample goes to
~/.cache/sumi/ai-samples/<label>.jsonl, and a summary is printed at the end.

  tools/ai-sample.py <label> [--seconds 120] [--interval 2] [--monitor ID]

<label> names what is on screen, e.g. "fps-apex", "racing-forza", "desktop".
Play full-screen on the monitor, in SDR (AI Visual pauses in HDR).
"""
import argparse
import collections
import json
import os
import subprocess
import sys
import time

BUS = ["busctl", "--user", "--json=short", "call", "io.github.JenAriDev.Sumi",
       "/io/github/JenAriDev/Sumi", "io.github.JenAriDev.Sumi1"]


def call(method, *args):
    sig = ["s"] if args else []
    out = subprocess.run(BUS + [method] + sig + list(args), capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip() or f"{method} failed")
    return json.loads(json.loads(out.stdout)["data"][0])


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("label")
    p.add_argument("--seconds", type=float, default=120)
    p.add_argument("--interval", type=float, default=2)
    p.add_argument("--monitor", help="monitor id (default: the first one sumid lists)")
    a = p.parse_args()

    monitor = a.monitor or next(iter(call("ListMonitors")), {}).get("id")
    if not monitor:
        sys.exit("sumid lists no monitor")
    outdir = os.path.join(os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache"), "sumi", "ai-samples")
    os.makedirs(outdir, exist_ok=True)
    path = os.path.join(outdir, f"{a.label}.jsonl")
    print(f"sampling {monitor} for {a.seconds:.0f} s → {path}  (Ctrl+C stops early)")

    targets, tops, n = collections.Counter(), collections.Counter(), 0
    game_sum = conf_sum = 0.0
    end = time.monotonic() + a.seconds
    with open(path, "a", encoding="utf-8") as log:
        try:
            while time.monotonic() < end:
                t0 = time.monotonic()
                try:
                    r = call("AiClassifyNow", monitor)
                except RuntimeError as e:
                    print(f"  error: {e}")
                    time.sleep(a.interval)
                    continue
                obs, ver = r.get("observation", {}), r.get("verdict", {})
                log.write(json.dumps({"t": time.time(), "label": a.label, **r}) + "\n")
                log.flush()
                n += 1
                target = ver.get("target") or "unchanged"
                targets[target] += 1
                top = max(obs.get("scores", [["?", 0]]), key=lambda s: s[1])
                tops[top[0]] += 1
                game_sum += obs.get("is_game", 0)
                conf_sum += obs.get("confidence", 0)
                print(f"  {n:3d}  top {top[0]:<8} {top[1]:5.1f}%  game {obs.get('is_game', 0) * 100:5.1f}%"
                      f"  conf {obs.get('confidence', 0) * 100:5.1f}%  → {target}")
                time.sleep(max(0.0, a.interval - (time.monotonic() - t0)))
        except KeyboardInterrupt:
            pass
    if n:
        print(f"\n{n} samples · mean game {game_sum / n * 100:.0f}% · mean confidence {conf_sum / n * 100:.0f}%")
        print("top class: " + ", ".join(f"{k} {v * 100 // n}%" for k, v in tops.most_common()))
        print("mode AI Visual would pick: " + ", ".join(f"{k} {v * 100 // n}%" for k, v in targets.most_common()))


if __name__ == "__main__":
    main()

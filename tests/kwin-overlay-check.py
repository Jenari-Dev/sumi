#!/usr/bin/env python3
"""Checks for tests/kwin-overlay.sh: the Wayland protocol trace of sumi-app (WAYLAND_DEBUG=client)
and the screenshots taken through KWin ScreenShot2.

    kwin-overlay-check.py <app.log> <screenshot-dir> <output-width> <output-height>

Exits 1 when a check fails."""
import json
import re
import sys

from PIL import Image

log, out, W, H = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
BG = (0x1C, 0x2A, 0x3A)
ORB = 96
failed = []


def check(ok, what, detail=""):
    print(("ok    " if ok else "FAIL  ") + what + (f"  [{detail}]" if detail else ""))
    if not ok:
        failed.append(what)


# ---- Protocol trace -------------------------------------------------------------------------
surfaces = {}   # layer surface id -> dict(surface, layer, scope, anchor, margin, size, zone, kbd)
created = []    # every layer surface ever created, in order (ids are reused across app runs)
regions = {}    # region id -> number of rectangles added
input_region = {}   # wl_surface id -> region id (or None)
req = re.compile(r"-> (\w+)#(\d+)\.(\w+)\((.*)\)")
for line in open(log, errors="replace"):
    m = req.search(line)
    if not m:
        continue
    iface, oid, name, args = m.group(1), int(m.group(2)), m.group(3), m.group(4)
    if name == "get_layer_surface":
        a = re.match(r"new id zwlr_layer_surface_v1#(\d+), wl_surface#(\d+), \S+, (\d+), \"(.*)\"", args)
        if a:
            surfaces[int(a.group(1))] = dict(surface=int(a.group(2)), layer=int(a.group(3)), scope=a.group(4))
            created.append(surfaces[int(a.group(1))])
    elif iface == "zwlr_layer_surface_v1" and oid in surfaces:
        nums = [int(x) for x in re.findall(r"-?\d+", args)]
        key = dict(set_anchor="anchor", set_margin="margin", set_size="size",
                   set_exclusive_zone="zone", set_keyboard_interactivity="kbd").get(name)
        if key:
            value = nums if len(nums) > 1 else nums[0]
            surfaces[oid][key] = value
            surfaces[oid].setdefault("first_" + key, value)   # what it was created with
    elif name == "create_region":
        a = re.search(r"wl_region#(\d+)", args)
        if a:
            regions[int(a.group(1))] = 0
    elif iface == "wl_region" and name == "add":
        regions[oid] = regions.get(oid, 0) + 1
    elif iface == "wl_surface" and name == "set_input_region":
        a = re.search(r"wl_region#(\d+)", args)
        input_region[oid] = int(a.group(1)) if a else None

layers = [s for s in created if s["scope"] == "sumi-ai"]
check(len(layers) >= 1, "layer-shell surfaces with scope sumi-ai exist", f"{len(layers)} found")
orbs = [s for s in layers if s.get("size") == [ORB, ORB]]
msgs = [s for s in layers if s.get("size") and s["size"][1] != ORB]
check(len(orbs) >= 1, "the orb is a 96x96 layer surface")
check(len(msgs) >= 1, "the message is its own layer surface", f"sizes {[s.get('size') for s in msgs]}")
for s in layers:
    check(s["layer"] == 3, "on the overlay layer", f"layer {s['layer']}")
    check(s.get("anchor") == 10, "anchored bottom + right", f"anchor {s.get('anchor')}")
    check(s.get("zone") == 0, "no exclusive zone", f"zone {s.get('zone')}")
    check(s.get("kbd") == 0, "no keyboard focus", f"interactivity {s.get('kbd')}")
if orbs:
    o = orbs[0]
    check(o["first_margin"] == [0, 32, 96, 0], "orb starts at right 32 / bottom 96", str(o["first_margin"]))
    r = input_region.get(o["surface"])
    check(r is None, "the orb surface keeps the default (full) input region", f"region {r}")
if msgs:
    m = msgs[0]
    check(m["first_margin"] == [0, 32 + ORB + 8, 96 + (ORB - 44) // 2, 0], "message sits left of the orb, centred on it", str(m["first_margin"]))
    r = input_region.get(m["surface"])
    check(r is not None and regions.get(r, 1) == 0, "the message surface has an empty input region (click-through)",
          f"region {r} with {regions.get(r)} rectangles")

# ---- Screenshots ----------------------------------------------------------------------------


def load(name):
    im = Image.open(f"{out}/overlay-{name}.png").convert("RGB")
    check(im.size == (W, H), f"{name}: capture is the whole output", str(im.size))
    return im


def diff(px):
    return max(abs(px[0] - BG[0]), abs(px[1] - BG[1]), abs(px[2] - BG[2]))


def bbox(im, box):
    x0, y0, x1, y1 = box
    xs, ys = [], []
    px = im.load()
    for y in range(y0, y1):
        for x in range(x0, x1):
            if diff(px[x, y]) > 12:
                xs.append(x)
                ys.append(y)
    return (min(xs), min(ys), max(xs), max(ys)) if xs else None


def energy(im, box):
    x0, y0, x1, y1 = box
    px = im.load()
    return sum(diff(px[x, y]) for y in range(y0, y1) for x in range(x0, x1)) / ((x1 - x0) * (y1 - y0))


NX, NY = 1700, 800   # where the drag puts the orb
orb_box = (W - 32 - ORB - 2, H - 96 - ORB - 2, W, H - 96 + 2)
msg_box = (W - 32 - ORB - 8 - 400, H - 96 - ORB, W - 32 - ORB - 8 + 4, H - 96)
cx, cy = W - 32 - ORB / 2, H - 96 - ORB / 2

idle = load("idle")
bb = bbox(idle, orb_box)
check(bb is not None, "idle: the orb is on screen above the full-screen window")
if bb:
    mx, my = (bb[0] + bb[2]) / 2, (bb[1] + bb[3]) / 2
    check(abs(mx - cx) <= 3 and abs(my - cy) <= 3, "idle: the orb is centred at the bottom-right margin position",
          f"bbox {bb}, centre ({mx:.0f},{my:.0f}), expected ({cx:.0f},{cy:.0f})")
    check(80 <= bb[2] - bb[0] + 1 <= ORB and 80 <= bb[3] - bb[1] + 1 <= ORB, "idle: the orb is about 90 px across", str(bb))
check(bbox(idle, msg_box) is None, "idle: no message at startup (the first mode is only remembered)")

sw = load("switched")
mb = bbox(sw, msg_box)
check(mb is not None, "switched: the message is shown")
if mb:
    check(abs(mb[2] - (W - 32 - ORB - 8 - 1)) <= 2, "switched: the message ends 8 px left of the orb", str(mb))
    check(abs((mb[1] + mb[3]) / 2 - cy) <= 2, "switched: the message is vertically centred on the orb", str(mb))
    check(mb[2] - mb[0] > 120 and 40 <= mb[3] - mb[1] + 1 <= 48, "switched: the message has a plausible size", str(mb))
check(energy(sw, orb_box) > energy(idle, orb_box), "switched: the red pulse brightens the orb",
      f"{energy(sw, orb_box):.1f} vs {energy(idle, orb_box):.1f}")

after = load("after")
check(bbox(after, msg_box) is None, "after: the message faded out again after its 4 s")
check(bbox(after, orb_box) is not None, "after: the orb stays")

# ---- Pointer: clicks, drag, double-click, restart --------------------------------------------
clicks = [tuple(map(int, l.split("GAMECLICK")[1].split())) for l in open(f"{out}/game.log", errors="replace") if "GAMECLICK" in l]
check(clicks[:1] == [(800, 500)], "control: a click on the bare window below arrives", str(clicks))
check(clicks[1:] == [(1680, 936)],
      "a click on the message reaches the window below; clicks on the orb, drags and double-clicks do not", str(clicks))

def around(x, y):
    return (x - ORB // 2 - 2, y - ORB // 2 - 2, x + ORB // 2 + 2, y + ORB // 2 + 2)


def centred(im, x, y, what):
    b = bbox(im, around(x, y))
    check(b is not None, what + ": orb present")
    if b:
        mx, my = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
        check(abs(mx - x) <= 10 and abs(my - y) <= 10, what + ": orb is where it was dragged to",
              f"centre ({mx:.0f},{my:.0f}), expected ({x},{y})")


dragged = load("dragged")
centred(dragged, NX, NY, "dragged")
check(bbox(dragged, orb_box) is None, "dragged: nothing left at the old place")
hidden = load("hidden")
check(bbox(hidden, around(NX, NY)) is None, "hidden: a double-click hid the orb")
returned = load("returned")
centred(returned, NX, NY, "returned")
check(bbox(returned, (NX - 400, NY - 30, NX - ORB // 2 - 4, NY + 30)) is not None,
      "returned: the next switch brought the message back next to the moved orb")
restored = load("restored")
centred(restored, NX, NY, "restored after a restart")
try:
    saved = json.loads(json.load(open(f"{out}/app.json"))["ai_bubble_pos"])["Virtual-0"]
except Exception as e:  # noqa: BLE001
    saved = {"error": str(e)}
check(abs(saved.get("right", -99) - (W - NX - ORB // 2)) <= 10 and abs(saved.get("bottom", -99) - (H - NY - ORB // 2)) <= 10,
      "the position is saved per connector in app.json", str(saved))

paused = load("paused")
check(bbox(paused, (NX - 400, NY - 30, NX - ORB // 2 - 4, NY + 30)) is None, "paused: no message")
e_rest = energy(restored, around(NX, NY))
e_paused = energy(paused, around(NX, NY))
check(0 < e_paused < 0.7 * e_rest, "paused: the orb is dimmed", f"{e_paused:.1f} vs {e_rest:.1f}")

print("RESULT:", "FAILED " + str(len(failed)) if failed else "all checks passed")
sys.exit(1 if failed else 0)

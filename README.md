# Sumi

Monitor settings for ASUS gaming monitors on Linux: GameVisual picture modes,
colour, HDR modes, OLED care, GamePlus, PIP/PBP, KVM, OSD language and
shortcuts, per-application modes, AI Visual and hot keys. Sumi talks to the
monitor over DDC/CI — the same channel the monitor's own OSD settings use —
and does not need a vendor driver, account or network access.

Sumi is an independent, clean-room project. It is not affiliated with,
endorsed by or supported by ASUSTeK Computer Inc. "ASUS", "ROG", "GameVisual",
"GamePlus" and other names are trademarks of their owners and are used only
to describe compatibility.

## Status

Developed and tested on a ROG Swift PG32UCDM3 (QD-OLED) on KDE Plasma 6
(Wayland) with the NVIDIA driver. Other ASUS models that use the same "new
VCP" scheme (VCP 0xEF major version 2) should work; features are enabled per
monitor from its own capability string. Older-scheme models are detected but
only get the standard MCCS controls for now.

Verified on the PG32UCDM3: every setting `sumi write-selftest --extended`
covers (31 changes, each read back and restored), the GUI against the live
daemon, AI Visual capture and classification, and the firmware check. Not yet
exercised on hardware: switching HDR modes and HDR10 ↔ Dolby Vision, pixel
cleaning, power saving, factory reset, input switching, PIP/PBP and KVM (each
asks for confirmation). Reports welcome.

## Requirements

- The `i2c-dev` kernel module (`echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf`).
- Read/write access to `/dev/i2c-*` for your user. The udev rule shipped with
  ddcutil (`60-ddcutil-i2c.rules`, `TAG+="uaccess"`) is the easiest way.
- DDC/CI enabled in the monitor's OSD (on by default).
- KDE Plasma 6 for App Tweaker, AI Visual, MultiFrame, hot keys, HDR
  switching, Target Mode and the desktop options. The CLI and the monitor
  controls work on any desktop.
- To build: Rust 1.85+, Qt 6 (Quick, Widgets, Labs Platform), KF6 I18n and
  gettext. At run time, layer-shell-qt puts the AI Visual bubble above
  full-screen games (without it the bubble is a plain stay-on-top window).

## Install

    contrib/install-user.sh
    systemctl --user enable --now sumid

`install-user.sh` builds a release and installs for the current user only:
`sumi`, `sumid` and `sumi-app` in `~/.local/bin`, the `sumid` systemd user
unit, the menu entry and icon, the translations and this manual. Run it again
after updating; it restarts `sumid` if it is running. Then open **Sumi** from
the application menu.

To remove it:

    contrib/uninstall-user.sh

That keeps your settings, the capability cache and the AI model;
`contrib/uninstall-user.sh --purge` deletes those too, along with Sumi's
entries in System Settings › Shortcuts.

## How it is put together

- **sumid** is a small user daemon. It owns the DDC/CI connection, applies and
  verifies every change, follows the focused app (App Tweaker), runs AI Visual,
  listens for hot keys and the gamepad, and talks to KDE. It offers a D-Bus API
  on the session bus (`io.github.JenAriDev.Sumi`).
- **sumi-app** is the window and tray icon. It only talks to `sumid`.
- **sumi** is the command-line tool, for scripts and troubleshooting.

Every write is read back and verified. Settings that blank the panel, re-plug
the display, move the USB hub to another computer or switch inputs ask for
confirmation first. While the window is closed, `sumid` does not poll the
monitor; it only acts on App Tweaker, AI Visual, hot keys and the gamepad.

## Using Sumi

The sidebar has the same pages as ASUS DisplayWidget Center, in the same order.

### GameVisual

The picture mode (Scenery, Racing, Cinema, RTS/RPG, FPS, sRGB, MOBA, User) and
its settings: brightness, contrast, saturation, colour temperature (with RGB
gains in User), Shadow Boost and the Blue Light Filter. Controls the monitor
doesn't allow in the current mode are greyed out, as on the OSD.

- **HDR** and **HDR Dolby Vision** tiles: while HDR is on (System Settings ›
  Display HDR, or KDE's display settings), pick an HDR10 mode (Cinema, Gaming,
  Console, True Black) or a Dolby Vision mode (Bright, Dark, Gaming, Source
  only). Switching between HDR10 and Dolby Vision makes the monitor re-plug
  the display, so Sumi asks first.
- **Compare**: hold it to see the settings from before your last change;
  release it to go back.
- **Import / Export** save one mode's settings as JSON in DisplayWidget
  Center's format, so files move between Sumi and the Windows app. Import
  checks the model and offers to switch to the file's mode.
- **Mode Sync** copies mode changes to other connected monitors of the same
  model.
- **Reset** resets the current mode (asks first).

### AI Visual

AI Visual picks the GameVisual mode from what is on the screen: about once a
second it looks at the monitor's picture, recognises a shooter, racing, RPG or
strategy game, a MOBA, scenery or office work, and switches to the matching
mode. Its decision rules are DisplayWidget's (smoothed scores, an "is this a
game" gate and a confidence gate). Choose which modes it may use and what to do
with everything else ("Remain unchanged" or a mode of your choice).

- The first time, press **Download** to fetch the model: OpenAI's CLIP
  ViT-B/32 image encoder (MIT licence, about 350 MB) from Hugging Face. It is
  checked against a SHA-256 hash and stored in `~/.local/share/sumi/models`.
  **Remove** deletes it. Nothing is downloaded unless you press the button.
- Private by design: screen images are taken through KWin, kept in memory
  only for the moment they are classified, and are never saved or sent
  anywhere. The model runs on the CPU, a fraction of a second per frame.
- It pauses while the screen is locked or the display is off, and while the
  monitor shows HDR. App Tweaker rules win over AI Visual.
- **Show Notifications** (on by default, as in DisplayWidget) puts a small
  ink-drop orb in the bottom-right corner of the monitor while AI Visual runs;
  when it switches modes, a "Switched to … mode" message appears beside it.
  Drag the orb to move it (the place is remembered); double-click it to hide
  it until the next switch. It never takes clicks meant for the game except
  on the orb itself. While the Sumi window and tray icon aren't running at
  all, `sumid` sends a desktop notification instead.
- With AI Visual off, the panel shows a live preview of what it would choose,
  without changing anything.
- KWin only gives screenshots to programs that are listed for it;
  `install-user.sh` installs that entry for `sumid`
  (`io.github.jenaridev.sumid.desktop`, hidden from the menu).

### GamePlus

Crosshair, timer, FPS counter and display alignment, one at a time, with
arrow buttons that move the active overlay.

### OLED Care

Pixel cleaning (about 6 minutes with the screen off; asks first), the
pixel-cleaning reminder, Screen Move and the panel-protection switches the
monitor supports (screen dimming, logo, taskbar and boundary detection,
outer and global dimming, uniform brightness).

**Target Mode** darkens every window except the one you are using, to save
the panel around it. It uses KWin's *Dim Inactive* effect.

### MultiScreen

- **MultiFrame**: pick one of DisplayWidget's 13 window layouts for each
  monitor. Sumi sets it up as KWin tiles: hold **Shift** while dragging a
  window to see the zones and drop it into one. (DisplayWidget offers three
  layouts at drop time; KWin has one tile layout per screen.)
- **PIP/PBP**: picture-in-picture or side-by-side with a second input, with a
  source and GameVisual mode for each window. PBP re-plugs the display, so
  Sumi asks first.
- **KVM**: move the monitor's USB hub between its upstream ports (asks first:
  your keyboard and mouse may move with it).

### HotKey

Sumi's actions appear in **System Settings › Shortcuts › Sumi**, where you
give them keys; none are bound by default. Turn on **Enable hot keys** on the
HotKey page first. The actions: brightness, contrast, saturation, Shadow
Boost and Blue Light Filter up/down, next/previous GameVisual mode, PIP/PBP,
KVM, power saving, Target Mode and HDR on/off, and **EZ OSD** — open the
monitor's own menu and drive it with keys (up, down, left, right, enter,
back, close). Hot keys act on the monitor that shows the active window.

**Gamepad EZ OSD**: with it switched on, hold **LB + RB + LT + RT** on a
controller to arm it; then Start opens the monitor menu, the D-pad moves, A
selects and B goes back. The same chord again (or a minute without input)
disarms it. Games still receive every button.

### App Tweaker

Give an app its own GameVisual mode: up to ten apps from your installed apps
(Steam games included) or the recently used list. When the app gets focus on
that monitor, Sumi switches the mode. **Default Setting** sets a mode for
every other app (or leaves it alone, or hands over to AI Visual). As in
DisplayWidget, when HDR is on and an app's mode is an SDR mode, Sumi turns
HDR off first; a switch on the page turns that off. Another switch restores
the previous mode when the app loses focus.

### System Settings

Display HDR (KDE's HDR for this monitor), power mode (Standard or Power
saving, which locks most settings), OSD language and the OSD shortcut keys,
input select and automatic input detection, volume and mute,
Adaptive-Sync (VRR), and two desktop options: auto-hide the Plasma panel and a
dark theme.

### User Center and Application Settings

User Center shows the monitor's model, firmware, refresh rate, usage time,
input, mode, HDR format and connection, and copies them as text.

**Firmware updates**: *Check for updates* asks ASUS's download server
(`dlcdnets.asus.com`, the same check DisplayWidget makes) whether newer
firmware exists for your model; only the model name and candidate version
numbers are in the request, and nothing is contacted unless you press the
button. *Download* saves the firmware zip to your Downloads folder. Sumi
doesn't flash firmware (DisplayWidget doesn't either): the zip holds ASUS's
Windows update tool, which you run on Windows. The CLI has the same check:
`sumi firmware check` / `sumi firmware download`.
Application Settings has the version, the language, start on login,
diagnostics export and this manual.

**Language**: Sumi speaks the 23 languages the monitor's menu has. Follow the
system language, follow the monitor's OSD language, or pick one; the window
restarts to apply it. `SUMI_LANGUAGE=ja sumi-app` overrides it for one run.

## Not included

These parts of DisplayWidget Center need ASUS's servers or an ASUS account,
and Sumi leaves them out: account sign-in, product registration, usage
telemetry, app updates, the online game list, the factory calibration
report and Smart Voice Control. Firmware is checked and downloaded (above)
but never flashed by Sumi.

## Troubleshooting

- **No supported monitor found**: check `sumi detect`. If the monitor is
  listed but doesn't answer, turn DDC/CI on in the OSD and check that
  `/dev/i2c-*` is readable (`ls -l /dev/i2c-*`).
- **Sumi can't reach its daemon**: `systemctl --user enable --now sumid`;
  logs with `journalctl --user -u sumid`.
- **A setting reverts**: some settings are locked by the current mode, HDR
  or power saving; the monitor refuses them and Sumi reports it.
- **AI Visual can't capture the screen**: run `contrib/install-user.sh` again;
  it installs the KWin entry and restarts `sumid`.
- **Slow responses on NVIDIA**: its i2c driver needs about 0.1 s per read;
  Sumi serialises and caches to keep the window quick.

## Command line

    sumi detect                     # monitors on GPU i2c buses
    sumi state                      # every supported setting, decoded
    sumi apply brightness 80 --confirm
    sumi probe                      # read-only: which VCP codes an OSD change moves
    sumi --json caps                # parsed capability string
    sumi ai status                  # AI Visual model
    sumi write-selftest --confirm   # change, verify and restore harmless settings

## Translations

Translations live in `po/` (gettext; one catalogue, `sumi`, for the app and
the daemon). `po/extract.sh` regenerates `po/sumi.pot` and merges it into the
`.po` files; `po/trwork.py check` reports what is untranslated.

## Parts

| Crate | What it is |
|---|---|
| `sumi-proto` | DDC/CI framing, MCCS capability strings, EDID |
| `sumi-ddc` | i2c-dev transport, monitor discovery, DRM connector mapping |
| `sumi-core` | feature catalogue, enable rules, verified writes, config, import/export, MultiFrame layouts |
| `sumi-ai` | AI Visual: capture, CLIP inference (tract), DisplayWidget's decision logic |
| `sumi` | command-line tool |
| `sumid` | user daemon on the session bus |
| `sumi-gui` | the desktop app `sumi-app` (Qt 6 / QML) |

## Licence

GPL-2.0-or-later. The AI Visual model is downloaded separately and is
MIT-licensed by OpenAI.

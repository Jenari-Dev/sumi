//! Gamepad EZ OSD (DisplayWidget's gamepad mode): hold LB+RB+LT+RT to arm,
//! then Start opens the monitor menu, the D-pad moves (repeating when held),
//! A selects and B/Back goes back. Same chord again (or a minute idle) disarms.
//! Devices are read without a grab, so games still receive every input.

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::manager::Manager;

const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const BTN_SOUTH: u16 = 0x130;
const BTN_EAST: u16 = 0x131;
const BTN_TL: u16 = 0x136;
const BTN_TR: u16 = 0x137;
const BTN_TL2: u16 = 0x138;
const BTN_TR2: u16 = 0x139;
const BTN_SELECT: u16 = 0x13a;
const BTN_START: u16 = 0x13b;
const BTN_DPAD_UP: u16 = 0x220;
const BTN_DPAD_DOWN: u16 = 0x221;
const BTN_DPAD_LEFT: u16 = 0x222;
const BTN_DPAD_RIGHT: u16 = 0x223;
const ABS_Z: u16 = 0x02;
const ABS_RZ: u16 = 0x05;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

const REPEAT_AFTER: Duration = Duration::from_millis(600);
const REPEAT_EVERY: Duration = Duration::from_millis(150);
const IDLE_DISARM: Duration = Duration::from_secs(60);

/// Game controllers: event nodes that report BTN_SOUTH (BTN_GAMEPAD).
pub fn gamepads() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/input") else { return out };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("event") {
            continue;
        }
        let dev = e.path().join("device");
        let Ok(caps) = std::fs::read_to_string(dev.join("capabilities/key")) else { continue };
        if reports_south_button(&caps) {
            let label = std::fs::read_to_string(dev.join("name")).unwrap_or_default().trim().to_string();
            out.push((PathBuf::from("/dev/input").join(&name), label));
        }
    }
    out.sort();
    out
}

/// Does a sysfs `capabilities/key` bitmap include BTN_SOUTH? The bitmap is
/// space-separated hex words, most significant first, 64 bits each.
fn reports_south_button(caps: &str) -> bool {
    let words: Vec<u64> = caps.split_whitespace().rev().filter_map(|w| u64::from_str_radix(w, 16).ok()).collect();
    let bit = BTN_SOUTH as usize;
    words.get(bit / 64).is_some_and(|w| w & (1 << (bit % 64)) != 0)
}

/// Maximum of an absolute axis (EVIOCGABS), for analog triggers.
fn abs_max(f: &File, axis: u16) -> i32 {
    let mut info = [0i32; 6];
    let req = 0x8018_4540 + axis as libc::c_ulong;
    if unsafe { libc::ioctl(f.as_raw_fd(), req, info.as_mut_ptr()) } == 0 { info[2] } else { 255 }
}

/// What the pad asks the daemon to do.
#[derive(Debug, PartialEq, Eq)]
enum Out {
    /// Run this hot-key action on the target monitor.
    Act(&'static str),
    /// The chord armed (`true`) or disarmed (`false`) the pad.
    Armed(bool),
    /// A minute without input disarmed it.
    IdleDisarmed,
}

/// One controller's EZ OSD state machine. It takes input events and the time
/// and says what to do; the device I/O is in `run_device`.
struct Pad {
    held: HashSet<u16>,
    lt_analog: bool,
    rt_analog: bool,
    /// Analog trigger range (EVIOCGABS), for the "pressed" threshold.
    lt_max: i32,
    rt_max: i32,
    armed: bool,
    /// The held direction: (action, pressed at, last repeat).
    dir: Option<(&'static str, Instant, Instant)>,
    last_event: Option<Instant>,
}

impl Pad {
    fn new(lt_max: i32, rt_max: i32) -> Self {
        Pad { held: HashSet::new(), lt_analog: false, rt_analog: false, lt_max, rt_max, armed: false, dir: None, last_event: None }
    }

    fn chord(&self) -> bool {
        let lt = self.held.contains(&BTN_TL2) || self.lt_analog;
        let rt = self.held.contains(&BTN_TR2) || self.rt_analog;
        self.held.contains(&BTN_TL) && self.held.contains(&BTN_TR) && lt && rt
    }

    fn disarm(&mut self) {
        self.armed = false;
        // A direction still held now is not held when it re-arms.
        self.dir = None;
    }

    /// The timer fired with no input: auto-repeat a held direction, disarm when idle.
    fn tick(&mut self, now: Instant) -> Vec<Out> {
        let mut out = Vec::new();
        if let Some((action, since, last)) = self.dir {
            if self.armed && now - since >= REPEAT_AFTER && now - last >= REPEAT_EVERY {
                out.push(Out::Act(action));
                self.dir = Some((action, since, now));
            }
        }
        if self.armed && self.last_event.is_some_and(|t| now - t > IDLE_DISARM) {
            self.disarm();
            out.push(Out::IdleDisarmed);
        }
        out
    }

    /// One input event (`ty`/`code`/`value` as evdev reports it). `enabled` is
    /// the config switch: while off, the pad disarms and ignores everything.
    fn event(&mut self, ty: u16, code: u16, value: i32, now: Instant, enabled: bool) -> Vec<Out> {
        if ty != EV_KEY && ty != EV_ABS {
            return Vec::new();
        }
        self.last_event = Some(now);
        if !enabled {
            self.disarm();
            return Vec::new();
        }
        let before = self.chord();
        let mut press: Option<&'static str> = None;
        let mut dir: Option<Option<&'static str>> = None;
        match (ty, code) {
            (EV_KEY, c) => {
                if value != 0 {
                    self.held.insert(c);
                } else {
                    self.held.remove(&c);
                }
                let down = value == 1;
                match c {
                    BTN_START if down => press = Some("osd_show"),
                    BTN_SOUTH if down => press = Some("osd_enter"),
                    BTN_EAST | BTN_SELECT if down => press = Some("osd_back"),
                    BTN_DPAD_UP => dir = Some(down.then_some("osd_up")),
                    BTN_DPAD_DOWN => dir = Some(down.then_some("osd_down")),
                    BTN_DPAD_LEFT => dir = Some(down.then_some("osd_left")),
                    BTN_DPAD_RIGHT => dir = Some(down.then_some("osd_right")),
                    _ => {}
                }
            }
            (EV_ABS, ABS_Z) => self.lt_analog = value > self.lt_max / 2,
            (EV_ABS, ABS_RZ) => self.rt_analog = value > self.rt_max / 2,
            (EV_ABS, ABS_HAT0X) => dir = Some(match value.signum() { -1 => Some("osd_left"), 1 => Some("osd_right"), _ => None }),
            (EV_ABS, ABS_HAT0Y) => dir = Some(match value.signum() { -1 => Some("osd_up"), 1 => Some("osd_down"), _ => None }),
            _ => {}
        }
        if !before && self.chord() {
            if self.armed {
                self.disarm();
            } else {
                self.armed = true;
            }
            return vec![Out::Armed(self.armed)];
        }
        if !self.armed {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Some(a) = press {
            out.push(Out::Act(a));
        }
        if let Some(d) = dir {
            self.dir = d.map(|a| (a, now, now));
            if let Some(a) = d {
                out.push(Out::Act(a));
            }
        }
        out
    }
}

fn run_device(mgr: Arc<Manager>, path: PathBuf, name: String, live: Arc<Mutex<HashSet<PathBuf>>>) {
    let result = (|| -> std::io::Result<()> {
        let mut f = File::open(&path)?;
        let mut pad = Pad::new(abs_max(&f, ABS_Z), abs_max(&f, ABS_RZ));
        let mut buf = [0u8; 24];
        let act = |action: &'static str| {
            let m = mgr.clone();
            std::thread::spawn(move || {
                if let Err(e) = m.hotkey(action) {
                    warn!("gamepad {action}: {e:#}");
                }
            });
        };
        let run = |outs: Vec<Out>| {
            for o in outs {
                match o {
                    Out::Act(a) => act(a),
                    Out::Armed(on) => info!("gamepad EZ OSD {} ({name})", if on { "armed" } else { "disarmed" }),
                    Out::IdleDisarmed => info!("gamepad EZ OSD disarmed (idle)"),
                }
            }
        };
        loop {
            let mut pfd = libc::pollfd { fd: f.as_raw_fd(), events: libc::POLLIN, revents: 0 };
            let ready = unsafe { libc::poll(&mut pfd, 1, REPEAT_EVERY.as_millis() as i32) };
            let now = Instant::now();
            if ready == 0 {
                run(pad.tick(now));
                continue;
            }
            if ready < 0 || pfd.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
                return Ok(()); // unplugged
            }
            f.read_exact(&mut buf)?;
            let ty = u16::from_ne_bytes([buf[16], buf[17]]);
            let code = u16::from_ne_bytes([buf[18], buf[19]]);
            let value = i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]);
            let enabled = mgr.config.lock().unwrap().gamepad.enabled;
            run(pad.event(ty, code, value, now, enabled));
        }
    })();
    if let Err(e) = result {
        warn!("gamepad {name}: {e}");
    }
    live.lock().unwrap().remove(&path);
}

/// Watch for controllers and start a reader per device.
pub fn spawn(mgr: Arc<Manager>) {
    let live: Arc<Mutex<HashSet<PathBuf>>> = Arc::default();
    std::thread::Builder::new()
        .name("gamepads".into())
        .spawn(move || loop {
            for (path, name) in gamepads() {
                if live.lock().unwrap().insert(path.clone()) {
                    info!("gamepad found: {name}");
                    let (m, l) = (mgr.clone(), live.clone());
                    std::thread::spawn(move || run_device(m, path, name, l));
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use sumi_core::hotkeys::ACTIONS;

    /// A pad plus a clock the test moves by hand.
    struct Rig {
        pad: Pad,
        t0: Instant,
        at: Duration,
    }

    impl Rig {
        fn new() -> Self {
            Rig { pad: Pad::new(255, 255), t0: Instant::now(), at: Duration::ZERO }
        }

        fn now(&self) -> Instant {
            self.t0 + self.at
        }

        fn wait(&mut self, ms: u64) {
            self.at += Duration::from_millis(ms);
        }

        fn event(&mut self, ty: u16, code: u16, value: i32) -> Vec<Out> {
            self.pad.event(ty, code, value, self.now(), true)
        }

        fn press(&mut self, code: u16) -> Vec<Out> {
            self.event(EV_KEY, code, 1)
        }

        fn release(&mut self, code: u16) -> Vec<Out> {
            self.event(EV_KEY, code, 0)
        }

        fn tick(&mut self) -> Vec<Out> {
            self.pad.tick(self.now())
        }

        /// Hold LB, RB, LT and RT (the triggers as buttons); the last press is returned.
        fn chord(&mut self) -> Vec<Out> {
            self.press(BTN_TL);
            self.press(BTN_TR);
            self.press(BTN_TL2);
            self.press(BTN_TR2)
        }

        fn release_chord(&mut self) {
            for b in [BTN_TL, BTN_TR, BTN_TL2, BTN_TR2] {
                self.release(b);
            }
        }

        fn armed() -> Self {
            let mut r = Rig::new();
            assert_eq!(r.chord(), [Out::Armed(true)]);
            r.release_chord();
            r
        }
    }

    fn act(a: &'static str) -> Vec<Out> {
        vec![Out::Act(a)]
    }

    #[test]
    fn the_four_shoulder_buttons_arm_it() {
        let mut r = Rig::new();
        assert!(r.press(BTN_TL).is_empty());
        assert!(r.press(BTN_TR).is_empty());
        assert!(r.press(BTN_TL2).is_empty());
        assert!(!r.pad.armed);
        assert_eq!(r.press(BTN_TR2), [Out::Armed(true)]);
        assert!(r.pad.armed);
    }

    #[test]
    fn three_of_the_four_do_not() {
        let mut r = Rig::new();
        for b in [BTN_TL, BTN_TR, BTN_TL2] {
            assert!(r.press(b).is_empty());
        }
        // Other buttons held with them do not complete the chord either.
        assert!(r.press(BTN_SOUTH).is_empty());
        assert!(r.press(BTN_START).is_empty());
        assert!(!r.pad.armed);
    }

    #[test]
    fn analog_triggers_count_past_half_travel() {
        let mut r = Rig::new();
        r.press(BTN_TL);
        r.press(BTN_TR);
        assert!(r.event(EV_ABS, ABS_Z, 255).is_empty());
        assert!(r.event(EV_ABS, ABS_RZ, 127).is_empty(), "half travel is not enough");
        assert_eq!(r.event(EV_ABS, ABS_RZ, 128), [Out::Armed(true)]);
        // Let go of one trigger and pull it again: that is a new chord, which disarms.
        r.event(EV_ABS, ABS_RZ, 0);
        r.event(EV_ABS, ABS_Z, 255);
        assert_eq!(r.event(EV_ABS, ABS_RZ, 255), [Out::Armed(false)]);
    }

    #[test]
    fn the_trigger_threshold_follows_the_axis_range() {
        let mut r = Rig::new();
        r.pad = Pad::new(1023, 1023);
        r.press(BTN_TL);
        r.press(BTN_TR);
        r.event(EV_ABS, ABS_Z, 600);
        assert!(r.event(EV_ABS, ABS_RZ, 500).is_empty());
        assert_eq!(r.event(EV_ABS, ABS_RZ, 600), [Out::Armed(true)]);
    }

    #[test]
    fn the_chord_again_disarms() {
        let mut r = Rig::armed();
        assert_eq!(r.chord(), [Out::Armed(false)]);
        assert!(!r.pad.armed);
        r.release_chord();
        assert_eq!(r.chord(), [Out::Armed(true)]);
    }

    #[test]
    fn nothing_happens_while_disarmed() {
        let mut r = Rig::new();
        for b in [BTN_START, BTN_SOUTH, BTN_EAST, BTN_SELECT, BTN_DPAD_UP, BTN_DPAD_DOWN] {
            assert!(r.press(b).is_empty());
            assert!(r.release(b).is_empty());
        }
        assert!(r.event(EV_ABS, ABS_HAT0X, 1).is_empty());
        r.wait(2000);
        assert!(r.tick().is_empty());
    }

    #[test]
    fn start_opens_the_menu_a_selects_b_and_back_go_back() {
        let mut r = Rig::armed();
        assert_eq!(r.press(BTN_START), act("osd_show"));
        assert_eq!(r.press(BTN_SOUTH), act("osd_enter"));
        assert_eq!(r.press(BTN_EAST), act("osd_back"));
        assert_eq!(r.press(BTN_SELECT), act("osd_back"));
        // Releases and key repeats (value 2) do nothing.
        assert!(r.release(BTN_START).is_empty());
        assert!(r.event(EV_KEY, BTN_SOUTH, 2).is_empty());
    }

    #[test]
    fn the_chord_buttons_do_nothing_else_once_armed() {
        let mut r = Rig::armed();
        for b in [BTN_TL, BTN_TR, BTN_TL2] {
            assert!(r.press(b).is_empty());
        }
    }

    #[test]
    fn the_dpad_moves_through_the_menu() {
        let mut r = Rig::armed();
        for (b, a) in [(BTN_DPAD_UP, "osd_up"), (BTN_DPAD_DOWN, "osd_down"), (BTN_DPAD_LEFT, "osd_left"), (BTN_DPAD_RIGHT, "osd_right")] {
            assert_eq!(r.press(b), act(a));
            assert!(r.release(b).is_empty());
        }
    }

    #[test]
    fn a_hat_axis_works_like_the_dpad() {
        let mut r = Rig::armed();
        assert_eq!(r.event(EV_ABS, ABS_HAT0X, -1), act("osd_left"));
        assert!(r.event(EV_ABS, ABS_HAT0X, 0).is_empty());
        assert_eq!(r.event(EV_ABS, ABS_HAT0X, 1), act("osd_right"));
        assert!(r.event(EV_ABS, ABS_HAT0X, 0).is_empty());
        assert_eq!(r.event(EV_ABS, ABS_HAT0Y, -1), act("osd_up"));
        assert!(r.event(EV_ABS, ABS_HAT0Y, 0).is_empty());
        assert_eq!(r.event(EV_ABS, ABS_HAT0Y, 1), act("osd_down"));
        assert!(r.pad.dir.is_some());
        r.event(EV_ABS, ABS_HAT0Y, 0);
        assert!(r.pad.dir.is_none());
    }

    #[test]
    fn a_held_direction_repeats_after_600_ms_every_150() {
        let mut r = Rig::armed();
        assert_eq!(r.press(BTN_DPAD_DOWN), act("osd_down"));
        r.wait(150);
        assert!(r.tick().is_empty());
        r.wait(449);
        assert!(r.tick().is_empty(), "599 ms in");
        r.wait(1);
        assert_eq!(r.tick(), act("osd_down"));
        r.wait(149);
        assert!(r.tick().is_empty());
        r.wait(1);
        assert_eq!(r.tick(), act("osd_down"));
        r.wait(150);
        assert_eq!(r.tick(), act("osd_down"));
        // Letting go stops it.
        r.release(BTN_DPAD_DOWN);
        r.wait(300);
        assert!(r.tick().is_empty());
    }

    #[test]
    fn changing_direction_restarts_the_repeat_delay() {
        let mut r = Rig::armed();
        r.press(BTN_DPAD_UP);
        r.wait(700);
        assert_eq!(r.tick(), act("osd_up"));
        r.release(BTN_DPAD_UP);
        assert_eq!(r.press(BTN_DPAD_RIGHT), act("osd_right"));
        r.wait(300);
        assert!(r.tick().is_empty());
        r.wait(300);
        assert_eq!(r.tick(), act("osd_right"));
    }

    #[test]
    fn a_minute_without_input_disarms() {
        let mut r = Rig::armed();
        r.wait(59_999);
        assert!(r.tick().is_empty());
        assert!(r.pad.armed);
        r.wait(2);
        assert_eq!(r.tick(), [Out::IdleDisarmed]);
        assert!(!r.pad.armed);
        // Once only.
        r.wait(5_000);
        assert!(r.tick().is_empty());
        assert!(r.press(BTN_START).is_empty());
    }

    #[test]
    fn input_keeps_it_armed() {
        let mut r = Rig::armed();
        for _ in 0..5 {
            r.wait(50_000);
            assert_eq!(r.press(BTN_START), act("osd_show"));
            assert!(r.tick().is_empty());
        }
        assert!(r.pad.armed);
        // Any button counts, even one that does nothing.
        r.wait(50_000);
        r.press(BTN_TR2);
        r.wait(50_000);
        assert!(r.tick().is_empty());
    }

    #[test]
    fn a_direction_held_through_a_disarm_does_not_come_back_on_rearming() {
        let mut r = Rig::armed();
        r.press(BTN_DPAD_UP);
        // The idle timeout runs out with the button still down (no events)…
        r.wait(61_000);
        assert!(r.tick().contains(&Out::IdleDisarmed));
        // …then it is released while disarmed, and the pad is armed again later.
        r.release(BTN_DPAD_UP);
        r.wait(1_000);
        assert_eq!(r.chord(), [Out::Armed(true)]);
        r.release_chord();
        r.wait(5_000);
        assert!(r.tick().is_empty(), "a stale direction repeated");
        assert!(r.pad.dir.is_none());
    }

    #[test]
    fn a_direction_held_while_the_chord_disarms_does_not_come_back_either() {
        let mut r = Rig::armed();
        r.press(BTN_DPAD_LEFT);
        assert_eq!(r.chord(), [Out::Armed(false)]);
        r.release(BTN_DPAD_LEFT);
        r.release_chord();
        assert_eq!(r.chord(), [Out::Armed(true)]);
        r.wait(2_000);
        assert!(r.tick().is_empty());
    }

    #[test]
    fn turning_the_option_off_disarms() {
        let mut r = Rig::armed();
        r.press(BTN_DPAD_UP);
        assert!(r.pad.event(EV_KEY, BTN_START, 1, r.now(), false).is_empty());
        assert!(!r.pad.armed);
        assert!(r.pad.dir.is_none());
        // While off the chord does not arm it, and presses are not tracked.
        for b in [BTN_TL, BTN_TR, BTN_TL2, BTN_TR2] {
            assert!(r.pad.event(EV_KEY, b, 1, r.now(), false).is_empty());
        }
        assert!(!r.pad.armed);
    }

    #[test]
    fn events_that_are_not_keys_or_axes_are_ignored() {
        let mut r = Rig::armed();
        r.wait(50_000);
        // EV_SYN / EV_MSC noise must not count as activity.
        assert!(r.event(0, 0, 0).is_empty());
        assert!(r.event(4, 4, 1).is_empty());
        r.wait(11_000);
        assert_eq!(r.tick(), [Out::IdleDisarmed]);
    }

    #[test]
    fn analog_stick_axes_do_not_press_anything() {
        let mut r = Rig::armed();
        for axis in [0x00, 0x01, 0x03, 0x04] {
            assert!(r.event(EV_ABS, axis, 30_000).is_empty());
        }
    }

    #[test]
    fn every_action_the_pad_sends_is_a_registered_hot_key() {
        let mut r = Rig::armed();
        let mut seen = std::collections::BTreeSet::new();
        for b in [BTN_START, BTN_SOUTH, BTN_EAST, BTN_SELECT, BTN_DPAD_UP, BTN_DPAD_DOWN, BTN_DPAD_LEFT, BTN_DPAD_RIGHT] {
            for o in r.press(b) {
                let Out::Act(a) = o else { panic!("{o:?}") };
                seen.insert(a);
            }
            r.release(b);
        }
        assert_eq!(seen.len(), 7, "{seen:?}");
        for a in seen {
            assert!(ACTIONS.iter().any(|(id, _)| *id == a), "{a} is not in hotkeys::ACTIONS");
        }
    }

    #[test]
    fn pads_are_found_by_their_south_button_bit() {
        // An Xbox-style pad, a keyboard and a mouse, as sysfs prints their key bitmaps.
        assert!(reports_south_button("7cdb000000000000 0 0 0 0"));
        assert!(!reports_south_button("402000000 3803078f800d001 feffffdfffefffff fffffffffffffffe"));
        assert!(!reports_south_button("1f0000 0 0 0 0"));
        // BTN_EAST alone is not it; BTN_SOUTH alone is.
        assert!(!reports_south_button("2000000000000 0 0 0 0"));
        assert!(reports_south_button("1000000000000 0 0 0 0"));
        assert!(!reports_south_button(""));
        assert!(!reports_south_button("not hex"));
    }
}

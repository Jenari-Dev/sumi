//! GameVisual settings import/export in DisplayWidget Center's JSON format
//! (`{"ModelName", "DisplayMode", "vcp": {"10": "80", …}}`), so files move
//! between the two apps.
//!
//! Import deliberately differs from the vendor where its raw replay is unsafe:
//! the packed toggle registers are applied bit by bit (never the pixel
//! cleaning trigger), and power saving / HDR mode / unknown codes (anything
//! outside DisplayWidget's own export list) are skipped.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

use crate::features;
use crate::monitor::{Monitor, ProductLine};

/// DisplayWidget's Gaming export list, in its order.
pub const GAMING_CODES: &[u8] = &[
    0xDC, 0xE2, 0x14, 0xE6, 0x10, 0x12, 0x16, 0x18, 0x1A, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x72, 0x87, 0x8A,
    0x90, 0xE0, 0xE1, 0xE3, 0xE5, 0xEA, 0xEE, 0xFC, 0xFD,
];

/// Codes import never writes (state, not settings, or unsafe to replay).
const SKIP_ON_IMPORT: &[u8] = &[0xDC, 0xE2, 0xE1, 0xEE];

fn product_line_index(l: ProductLine) -> u8 {
    match l {
        ProductLine::Gaming => 0,
        ProductLine::MainStream => 1,
        ProductLine::Portable => 2,
        ProductLine::ProArt => 3,
        ProductLine::Unknown => 0,
    }
}

/// Bits of a packed register that import may set: known, supported, not triggers.
fn settable_bits(m: &Monitor, code: u8) -> u16 {
    let p = m.profile();
    features::FEATURES
        .iter()
        .filter(|f| f.code == code && matches!(f.encoding, features::Encoding::Bit { .. }) && p.supported(f))
        .map(features::bit_mask)
        .fold(0, |a, b| a | b)
}

impl Monitor {
    /// Read the current mode's settings as a DisplayWidget-compatible document.
    pub fn export_profile(&mut self) -> Result<Value> {
        let mode = self.get(0xDC).context("reading the GameVisual mode")?.current & 0xFF;
        let mut vcp = Map::new();
        let mut ui = Map::new();
        for &code in GAMING_CODES {
            if !self.caps.supports(code) && code != 0xDC {
                continue;
            }
            let Ok(r) = self.get(code) else { continue };
            if r.max == 0 || r.max == 0xFE {
                continue;
            }
            vcp.insert(format!("{code:02X}"), Value::String(r.current.to_string()));
            let as_f = r.current as f64;
            let key = match code {
                0x10 => "Brightness",
                0x12 => "Contrast",
                0xE6 => "BlueFilter",
                0x8A => "Saturation",
                0x16 => "RedGain",
                0x18 => "GreenGain",
                0x1A => "BlueGain",
                0x87 => "Sharpness",
                _ => "",
            };
            if !key.is_empty() {
                ui.insert(key.into(), json!(as_f));
            }
            match code {
                0xE5 => {
                    ui.insert("ShadowBoost".into(), json!(r.current));
                }
                0x72 => {
                    ui.insert("Gamma".into(), json!(r.current >> 8));
                }
                _ => {}
            }
        }
        let mut doc = Map::new();
        doc.insert("ModelName".into(), json!(self.model()));
        doc.insert("ProductLine".into(), json!(product_line_index(self.line)));
        doc.insert("DisplayMode".into(), json!(mode));
        doc.extend(ui);
        doc.insert("vcp".into(), Value::Object(vcp));
        Ok(Value::Object(doc))
    }

    /// Apply a document written by `export_profile` or DisplayWidget. With
    /// `switch_mode` the file's GameVisual mode is selected first; otherwise a
    /// mode mismatch is an error (DisplayWidget asks the user to switch).
    /// Returns one line per code.
    pub fn import_profile(&mut self, doc: &Value, switch_mode: bool) -> Result<Vec<String>> {
        let model = doc.get("ModelName").and_then(Value::as_str).unwrap_or_default();
        if !model.eq_ignore_ascii_case(self.model()) {
            bail!("this file is for {model:?}, not {}", self.model());
        }
        let vcp = doc.get("vcp").and_then(Value::as_object).context("no \"vcp\" section (old-format file)")?;
        let get = |code: u8| -> Option<u16> { vcp.get(&format!("{code:02X}"))?.as_str()?.trim().parse().ok() };
        // A packed register that cannot be used as the base of an update (a read
        // fails, or pixel cleaning is running) refuses the import before anything
        // is written, rather than leaving it half applied.
        for code in [0xFCu8, 0xFD] {
            if get(code).is_some() && self.caps.supports(code) && settable_bits(self, code) != 0 {
                self.read_packed(code)?;
            }
        }
        let file_mode = doc.get("DisplayMode").and_then(Value::as_u64).map(|m| m as u16).or_else(|| get(0xDC).map(|v| v & 0xFF));
        let cur_mode = self.get(0xDC)?.current & 0xFF;
        let mut log = Vec::new();
        if let Some(fm) = file_mode.filter(|&fm| fm != cur_mode) {
            if !switch_mode {
                bail!("the file is for GameVisual mode {fm}, the monitor is in mode {cur_mode}; switch first");
            }
            self.set_verified(0xDC, fm, Duration::from_millis(600))?;
            self.display.pause(Duration::from_secs(3));
            log.push(format!("0xDC GameVisual -> {fm}"));
        }
        let user_ct = get(0x14) == Some(0x0B);
        let blf = get(0xE6).filter(|&v| v != 0);
        if blf.is_some() {
            // Blue Light Filter changes brightness/blue gain: zero it first, restore it last.
            self.set_verified(0xE6, 0, Duration::from_millis(100))?;
        } else if get(0xE6) == Some(0) && self.caps.supports(0xE6) && self.get(0xE6).is_ok_and(|r| r.current != 0) {
            // The file has it off: switch off a filter that is on now (the loop skips 0xE6).
            match self.set_verified(0xE6, 0, Duration::from_millis(100)) {
                Ok(_) => log.push("0xE6 = 0".into()),
                Err(e) => log.push(format!("0xE6 failed: {e:#}")),
            }
        }
        let mut order: Vec<u8> = vcp.keys().filter_map(|k| u8::from_str_radix(k, 16).ok()).collect();
        // File order is not guaranteed by JSON parsers; use the vendor order.
        order.sort_by_key(|c| GAMING_CODES.iter().position(|g| g == c).unwrap_or(usize::MAX));
        for code in order {
            let Some(mut v) = get(code) else { continue };
            if SKIP_ON_IMPORT.contains(&code) || code == 0xE6 || !GAMING_CODES.contains(&code) || !self.caps.supports(code) {
                continue;
            }
            if matches!(code, 0x16 | 0x18 | 0x1A) && !user_ct {
                continue;
            }
            let r = match code {
                0xFC | 0xFD => {
                    let mask = settable_bits(self, code);
                    if mask == 0 {
                        continue;
                    }
                    let cur = match self.read_packed(code) {
                        Ok(r) => r.current,
                        Err(e) => {
                            log.push(format!("0x{code:02X} failed: {e:#}"));
                            continue;
                        }
                    };
                    let want = (cur & !mask) | (v & mask);
                    if want == cur {
                        log.push(format!("0x{code:02X} unchanged"));
                        continue;
                    }
                    self.set_verified_with(code, want, Duration::from_millis(100), |r| r.current & mask == want & mask)
                }
                _ => {
                    if code == 0x72 && v <= 0xFF {
                        v <<= 8; // legacy files store the gamma byte unshifted
                    }
                    self.set_verified(code, v, Duration::from_millis(100))
                }
            };
            match r {
                Ok(_) => log.push(format!("0x{code:02X} = {v}")),
                Err(e) => log.push(format!("0x{code:02X} failed: {e:#}")),
            }
            self.display.pause(Duration::from_millis(100));
        }
        if let Some(b) = blf {
            self.set_verified(0xE6, b, Duration::from_millis(100))?;
            log.push(format!("0xE6 = {b}"));
            self.display.pause(Duration::from_secs(2));
            for code in [0x10u8, 0x1A] {
                if code == 0x1A && !user_ct {
                    continue;
                }
                if let Some(v) = get(code) {
                    let _ = self.set_verified(code, v, Duration::from_millis(100));
                }
            }
        }
        Ok(log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{self, Fake};

    /// A DisplayWidget-style document: values are decimal strings under hex keys.
    fn doc(model: &str, mode: u16, vcp: &[(&str, u32)]) -> Value {
        let vcp: Map<String, Value> = vcp.iter().map(|(k, v)| (k.to_string(), Value::String(v.to_string()))).collect();
        json!({ "ModelName": model, "ProductLine": 0, "DisplayMode": mode, "vcp": vcp })
    }

    fn pg(vcp: &[(&str, u32)]) -> Value {
        doc("PG32UCDM3", 7, vcp)
    }

    fn import(m: &mut Monitor, d: &Value) -> Vec<String> {
        m.import_profile(d, false).unwrap()
    }

    // Export

    #[test]
    fn export_reads_the_current_mode_in_the_vendor_shape() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0x72, 0x7800);
        let d = m.export_profile().unwrap();
        assert_eq!(d["ModelName"], "PG32UCDM3");
        assert_eq!(d["ProductLine"], 0);
        assert_eq!(d["DisplayMode"], 7);
        // Decimal strings under upper-case hex keys; ShadowBoost and Gamma have their own keys.
        assert_eq!(d["vcp"]["10"], "100");
        assert_eq!(d["vcp"]["72"], "30720");
        assert_eq!(d["vcp"]["FD"], "30761");
        assert_eq!(d["Brightness"], 100.0);
        assert_eq!(d["Contrast"], 80.0);
        assert_eq!(d["ShadowBoost"], 3);
        assert_eq!(d["Gamma"], 0x78);
        assert_eq!(d["BlueFilter"], 0.0);
        // Codes the monitor lacks (0x90, 0xE0) or reports with a maximum of 0 (0xEE) are left out.
        for code in ["90", "E0", "EE"] {
            assert!(d["vcp"].get(code).is_none(), "{code}");
        }
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn export_needs_the_mode_but_skips_unreadable_codes() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.fail_reads(0x12);
        let d = m.export_profile().unwrap();
        assert!(d["vcp"].get("12").is_none() && d["vcp"].get("10").is_some());
        fake.fail_reads(0xDC);
        assert!(format!("{:#}", m.export_profile().unwrap_err()).contains("reading the GameVisual mode"));
    }

    #[test]
    fn export_uses_the_product_line_index() {
        assert_eq!(product_line_index(ProductLine::Gaming), 0);
        assert_eq!(product_line_index(ProductLine::MainStream), 1);
        assert_eq!(product_line_index(ProductLine::Portable), 2);
        assert_eq!(product_line_index(ProductLine::ProArt), 3);
    }

    // Round trip

    /// A monitor with every exportable setting moved away from the idle values.
    fn tweaked() -> (Monitor, Fake) {
        let (m, fake) = testing::pg32ucdm3();
        for (code, v) in [
            (0x10, 70),
            (0x12, 60),
            (0x14, 0x0B),
            (0x16, 90),
            (0x18, 80),
            (0x1A, 75),
            (0x59, 40),
            (0x5F, 3),
            (0x72, 0x7800),
            (0x87, 30),
            (0x8A, 40),
            (0xE3, 9),
            (0xE5, 1),
            (0xE6, 2),
            (0xFC, 0x0801),
            (0xFD, 0x7849),
        ] {
            fake.set_current(code, v);
        }
        (m, fake)
    }

    const ROUND_TRIP_CODES: &[u8] = &[
        0x10, 0x12, 0x14, 0x16, 0x18, 0x1A, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x72, 0x87, 0x8A, 0xE3, 0xE5, 0xE6, 0xEA, 0xFC, 0xFD,
    ];

    #[test]
    fn export_then_import_restores_the_settings() {
        let (mut a, source) = tweaked();
        let d = a.export_profile().unwrap();
        // The file survives a trip through text, as it would on disk.
        let d: Value = serde_json::from_str(&serde_json::to_string_pretty(&d).unwrap()).unwrap();
        let (mut b, target) = testing::pg32ucdm3();
        let log = import(&mut b, &d);
        for &code in ROUND_TRIP_CODES {
            assert_eq!(target.current(code), source.current(code), "0x{code:02X}");
        }
        assert!(log.iter().all(|l| !l.contains("failed")), "{log:?}");
        // What import must not touch stays as it was.
        for code in [0xE1, 0xE2, 0xDC, 0xEE] {
            assert!(target.writes().iter().all(|w| w.0 != code), "0x{code:02X}");
        }
        // Importing the same file again changes nothing in the packed registers.
        target.clear_logs();
        let again = import(&mut b, &d);
        assert!(again.contains(&"0xFD unchanged".to_string()) && again.contains(&"0xFC unchanged".to_string()), "{again:?}");
        assert!(target.writes().iter().all(|w| w.0 != 0xFD && w.0 != 0xFC));
    }

    #[test]
    fn import_order_follows_the_vendor_list_not_the_file() {
        let (mut m, fake) = testing::pg32ucdm3();
        // Reversed on purpose: JSON parsers don't keep file order.
        import(&mut m, &pg(&[("8A", 41), ("72", 0x7800), ("12", 61), ("10", 71), ("14", 6)]));
        let order: Vec<u8> = fake.writes().iter().map(|w| w.0).collect();
        assert_eq!(order, [0x14, 0x10, 0x12, 0x72, 0x8A]);
    }

    // Safety

    #[test]
    fn import_never_sets_the_pixel_cleaning_bit() {
        let (mut m, fake) = testing::pg32ucdm3();
        // A file with bit 4 set (a trigger) and bits 2 and 7 (not toggles Sumi knows).
        let file = 0x7879 | 0x0004 | 0x0080;
        let log = import(&mut m, &pg(&[("FD", file)]));
        // The log shows the file's word; what is written is only the known toggles.
        assert_eq!(log, [format!("0xFD = {file}")]);
        assert_eq!(fake.writes(), [(0xFD, 0x7869)]);
        assert_eq!(fake.current(0xFD) & 0x0010, 0);
        // Whatever the file says, bit 4 is never in a write.
        for v in [0u32, 0xFFFF, 0x0010, 0x7879, 0x7839] {
            fake.clear_logs();
            import(&mut m, &pg(&[("FD", v)]));
            assert!(fake.writes().iter().all(|(_, w)| w & 0x0010 == 0), "{v:#x}: {:?}", fake.writes());
        }
    }

    #[test]
    fn import_skips_power_saving_hdr_and_codes_outside_the_vendor_list() {
        let (mut m, fake) = testing::pg32ucdm3();
        let d = pg(&[
            ("E1", 1),      // power saving locks most features
            ("E2", 0x0102), // HDR mode: the compositor owns that
            ("EE", 1),      // old-scheme power saving
            ("DC", 7),
            ("99", 5),      // not in the capability string
            ("04", 1),      // restore factory defaults
            ("05", 1),      // restore brightness/contrast
            ("60", 0x11),   // input source
            ("62", 0),      // volume
            ("8D", 1),      // mute
            ("CC", 6),      // OSD language
            ("D6", 5),      // power mode: off
            ("F3", 3),      // KVM
            ("F4", 5),      // PIP/PBP
            ("EC", 0xFF),   // reset all
            ("10", 55),
        ]);
        let log = import(&mut m, &d);
        assert_eq!(fake.writes(), [(0x10, 55)]);
        assert_eq!(log, ["0x10 = 55"]);
    }

    #[test]
    fn import_ignores_keys_that_are_not_codes() {
        let (mut m, fake) = testing::pg32ucdm3();
        let mut d = pg(&[("10", 55)]);
        d["vcp"]["ZZ"] = json!("1");
        d["vcp"]["1FF"] = json!("1");
        d["vcp"]["12"] = json!("not a number");
        d["vcp"]["87"] = json!(40); // a number, not a string
        import(&mut m, &d);
        assert_eq!(fake.writes(), [(0x10, 55)]);
    }

    #[test]
    fn import_refuses_a_packed_register_it_cannot_use_before_writing_anything() {
        let (mut m, fake) = testing::pg32ucdm3();
        let d = pg(&[("10", 55), ("E6", 2), ("FD", 0x7869)]);
        // Pixel cleaning is running.
        fake.set_current(0xFD, 0x7839);
        let e = m.import_profile(&d, false).unwrap_err().to_string();
        assert!(e.contains("pixel cleaning is running"), "{e}");
        assert!(fake.writes().is_empty());
        // The register cannot be read.
        fake.set_current(0xFD, 0x7829);
        fake.fail_reads(0xFD);
        assert!(m.import_profile(&d, false).is_err());
        assert!(fake.writes().is_empty());
        // A file that does not touch 0xFD imports fine meanwhile.
        fake.set_current(0xFD, 0x7839);
        fake.heal_reads(0xFD);
        import(&mut m, &pg(&[("10", 55)]));
        assert_eq!(fake.writes(), [(0x10, 55)]);
        assert_eq!(fake.current(0xFD), 0x7839);
    }

    #[test]
    fn import_leaves_the_blue_light_filter_restored_when_a_packed_read_fails_midway() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xE6, 2);
        // The up-front check passes; the read in the loop does not.
        fake.fail_reads_after(0xFD, 1);
        let log = import(&mut m, &pg(&[("10", 55), ("E6", 2), ("FD", 0x7869)]));
        assert!(log.iter().any(|l| l.starts_with("0xFD failed")), "{log:?}");
        assert!(fake.writes().iter().all(|w| w.0 != 0xFD));
        assert_eq!(fake.current(0xE6), 2);
        assert_eq!(fake.writes().last(), Some(&(0x10, 55)));
    }

    #[test]
    fn import_refuses_another_monitors_file() {
        let (mut m, fake) = testing::pg32ucdm3();
        let e = m.import_profile(&doc("PG27UCDM", 7, &[("10", 55)]), false).unwrap_err().to_string();
        assert_eq!(e, "this file is for \"PG27UCDM\", not PG32UCDM3");
        assert!(m.import_profile(&json!({ "vcp": {} }), false).is_err());
        assert!(m.import_profile(&json!(null), false).is_err());
        // Nothing was read or written for it.
        assert!(fake.writes().is_empty() && fake.reads().is_empty());
        // The name is compared without regard to case.
        m.import_profile(&doc("pg32ucdm3", 7, &[("10", 55)]), false).unwrap();
        assert_eq!(fake.writes(), [(0x10, 55)]);
    }

    #[test]
    fn import_needs_the_vcp_section() {
        let (mut m, fake) = testing::pg32ucdm3();
        let old_format = json!({ "ModelName": "PG32UCDM3", "DisplayMode": 7, "Brightness": 50.0 });
        let e = m.import_profile(&old_format, false).unwrap_err().to_string();
        assert!(e.contains("old-format"), "{e}");
        assert!(fake.writes().is_empty());
    }

    // Values

    #[test]
    fn import_shifts_legacy_gamma() {
        let (mut m, fake) = testing::pg32ucdm3();
        let log = import(&mut m, &pg(&[("72", 0x78)]));
        assert_eq!(fake.writes(), [(0x72, 0x7800)]);
        assert_eq!(log, ["0x72 = 30720"]);
        // Already shifted: left alone.
        fake.clear_logs();
        import(&mut m, &pg(&[("72", 0x8C00)]));
        assert_eq!(fake.writes(), [(0x72, 0x8C00)]);
        // The boundary is 0xFF.
        fake.clear_logs();
        import(&mut m, &pg(&[("72", 0xFF)]));
        assert_eq!(fake.writes(), [(0x72, 0xFF00)]);
        // Other registers are never shifted.
        fake.clear_logs();
        import(&mut m, &pg(&[("87", 30)]));
        assert_eq!(fake.writes(), [(0x87, 30)]);
    }

    #[test]
    fn import_leaves_packed_registers_that_already_match() {
        let (mut m, fake) = testing::pg32ucdm3();
        // 0xFD differs from the monitor only in bits Sumi has no toggle for; 0xFC only in a bit it does not own.
        let log = import(&mut m, &pg(&[("FC", 0x0811 | 0x0004), ("FD", 0x7829 | 0x0004)]));
        assert_eq!(log, ["0xFC unchanged", "0xFD unchanged"]);
        assert!(fake.writes().is_empty());
        // A real difference is written once, and only in the toggles' bits.
        let log = import(&mut m, &pg(&[("FD", 0x7829 ^ 0x0040)]));
        assert_eq!(log, [format!("0xFD = {}", 0x7829 ^ 0x0040)]);
        assert_eq!(fake.writes(), [(0xFD, 0x7869)]);
    }

    #[test]
    fn import_checks_each_register_was_taken() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.ignore_writes(0x12);
        let log = import(&mut m, &pg(&[("10", 55), ("12", 50), ("87", 30)]));
        assert!(log[1].starts_with("0x12 failed: VCP 0x12: wrote 50, monitor reports 80"), "{log:?}");
        // One failure does not stop the rest.
        assert_eq!((log[0].as_str(), log[2].as_str()), ("0x10 = 55", "0x87 = 30"));
    }

    #[test]
    fn colour_gains_are_applied_only_for_user_colour_temperature() {
        let (mut m, fake) = testing::pg32ucdm3();
        import(&mut m, &pg(&[("14", 5), ("16", 77), ("18", 77), ("1A", 77)]));
        assert_eq!(fake.writes(), [(0x14, 5)]);
        fake.clear_logs();
        import(&mut m, &pg(&[("14", 0x0B), ("16", 77), ("18", 66), ("1A", 55)]));
        assert_eq!(fake.writes(), [(0x14, 0x0B), (0x16, 77), (0x18, 66), (0x1A, 55)]);
    }

    // Mode and blue light filter

    #[test]
    fn a_file_for_another_mode_needs_the_switch() {
        let (mut m, fake) = testing::pg32ucdm3();
        let d = doc("PG32UCDM3", 5, &[("10", 55)]);
        let e = m.import_profile(&d, false).unwrap_err().to_string();
        assert_eq!(e, "the file is for GameVisual mode 5, the monitor is in mode 7; switch first");
        assert!(fake.writes().is_empty());
        let log = m.import_profile(&d, true).unwrap();
        assert_eq!(fake.writes(), [(0xDC, 5), (0x10, 55)]);
        assert_eq!(log[0], "0xDC GameVisual -> 5");
        // The mode gets its settle time and the monitor three seconds to reload.
        assert_eq!(fake.pauses()[..2], [Duration::from_millis(600), Duration::from_secs(3)]);
    }

    #[test]
    fn the_mode_comes_from_the_dc_register_when_the_file_has_no_display_mode() {
        let (mut m, fake) = testing::pg32ucdm3();
        let mut d = pg(&[("DC", 0x0105), ("10", 55)]);
        d.as_object_mut().unwrap().remove("DisplayMode");
        assert!(m.import_profile(&d, false).is_err());
        m.import_profile(&d, true).unwrap();
        assert_eq!(fake.writes()[0], (0xDC, 5));
        // 0xDC itself is never replayed by the loop.
        assert_eq!(fake.writes().iter().filter(|w| w.0 == 0xDC).count(), 1);
    }

    #[test]
    fn the_blue_light_filter_is_zeroed_first_and_restored_last() {
        let (mut m, fake) = testing::pg32ucdm3();
        import(&mut m, &pg(&[("14", 0x0B), ("E6", 3), ("10", 40), ("1A", 75), ("12", 60)]));
        assert_eq!(
            fake.writes(),
            [(0xE6, 0), (0x14, 0x0B), (0x10, 40), (0x12, 60), (0x1A, 75), (0xE6, 3), (0x10, 40), (0x1A, 75)]
        );
        // The monitor needs two seconds before brightness and blue gain are written again.
        assert!(fake.pauses().contains(&Duration::from_secs(2)));
        assert_eq!(fake.current(0xE6), 3);
    }

    #[test]
    fn blue_gain_is_not_rewritten_after_the_filter_without_user_colour() {
        let (mut m, fake) = testing::pg32ucdm3();
        import(&mut m, &pg(&[("14", 5), ("E6", 2), ("10", 40), ("1A", 75)]));
        assert_eq!(fake.writes(), [(0xE6, 0), (0x14, 5), (0x10, 40), (0xE6, 2), (0x10, 40)]);
    }

    #[test]
    fn a_file_with_the_filter_off_switches_it_off() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xE6, 3);
        let log = import(&mut m, &pg(&[("E6", 0), ("10", 40)]));
        assert_eq!(fake.writes(), [(0xE6, 0), (0x10, 40)]);
        assert_eq!(log, ["0xE6 = 0", "0x10 = 40"]);
        // Already off: no write.
        fake.clear_logs();
        import(&mut m, &pg(&[("E6", 0), ("10", 40)]));
        assert_eq!(fake.writes(), [(0x10, 40)]);
        // If the monitor won't take it (it only does in Racing), say so and carry on.
        fake.set_current(0xE6, 3);
        fake.ignore_writes(0xE6);
        fake.clear_logs();
        let log = import(&mut m, &pg(&[("E6", 0), ("10", 40)]));
        assert!(log[0].starts_with("0xE6 failed"), "{log:?}");
        assert_eq!(log[1], "0x10 = 40");
    }
}

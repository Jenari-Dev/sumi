//! Reading a monitor's feature state and applying feature writes.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{bail, Context as _, Result};
use serde::Serialize;
use sumi_proto::VcpReply;

use crate::features::{self, bit_mask, Context, Encoding, Feature, Opt, Page, Profile, Risk};
use crate::monitor::{FirmwareInfo, Monitor, ProductLine};

#[derive(Debug, Clone, Serialize)]
pub struct FeatureState {
    pub id: &'static str,
    pub label: &'static str,
    pub page: Page,
    pub code: u8,
    pub encoding: Encoding,
    pub risk: Risk,
    pub vendor_ui: bool,
    /// Decoded value (byte, bit as 0/1, or the whole word); `None` if unreadable or write-only.
    pub value: Option<u16>,
    /// Range maximum (whole word) where meaningful.
    pub max: Option<u16>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Opt>,
    /// Present on this monitor. Unsupported features DisplayWidget still shows
    /// (greyed) are listed with `supported: false` so a UI can match it.
    pub supported: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorState {
    pub id: String,
    pub model: String,
    pub line: ProductLine,
    pub oled: bool,
    pub firmware: FirmwareInfo,
    pub bus: u32,
    /// DRM connector ("DP-3").
    pub connector: Option<String>,
    /// HDR format the monitor currently advertises in its EDID, from the
    /// kernel's copy (no DDC traffic): "dolby_vision" when the EDID carries a
    /// Dolby Vision block, else "hdr10". Switching it re-plugs the display.
    pub hdr_format: Option<&'static str>,
    pub context: Context,
    pub features: Vec<FeatureState>,
}

/// "dolby_vision" if the EDID carries a Dolby Vision vendor-specific video
/// data block (OUI 00-D0-46, stored LSB first), else "hdr10".
fn edid_hdr_format(edid: &[u8]) -> &'static str {
    if edid.windows(3).any(|w| w == [0x46, 0xD0, 0x00]) { "dolby_vision" } else { "hdr10" }
}

impl Monitor {
    pub fn profile(&self) -> Profile<'_> {
        Profile { caps: &self.caps, line: self.line, model: self.model(), new_vcp: self.fw.new_vcp() }
    }

    /// "dolby_vision" / "hdr10" from the connector's current EDID.
    pub fn hdr_format(&self) -> Option<&'static str> {
        let conn = self.found.connector.as_deref()?;
        let (_, edid) = self.display.output_edids().into_iter().find(|(c, _)| c == conn)?;
        Some(edid_hdr_format(&edid))
    }

    /// Features this monitor supports, optionally limited to some pages.
    pub fn features(&self, pages: Option<&[Page]>) -> Vec<&'static Feature> {
        let p = self.profile();
        features::FEATURES
            .iter()
            .filter(|f| pages.is_none_or(|ps| ps.contains(&f.page)))
            .filter(|f| p.supported(f))
            .collect()
    }

    /// Read the codes behind the given features (each code once).
    fn read_codes(&mut self, feats: &[&Feature]) -> BTreeMap<u8, Option<VcpReply>> {
        let mut codes: Vec<u8> = feats.iter().filter(|f| !matches!(f.encoding, Encoding::Command)).map(|f| f.code).collect();
        // Context codes, so enable rules are always right.
        codes.extend_from_slice(&[0xDC, 0xE2, 0xE1, 0x14, 0xE6]);
        codes.sort();
        codes.dedup();
        codes.into_iter().map(|c| (c, self.display.get_vcp(c).ok())).collect()
    }

    pub fn state(&mut self, pages: Option<&[Page]>) -> MonitorState {
        let feats = self.features(pages);
        let raw = self.read_codes(&feats);
        let cur = |c: u8| raw.get(&c).copied().flatten();
        let context = Context::from_regs(self.fw.new_vcp(), cur);
        let profile = self.profile();
        let greyed = features::FEATURES
            .iter()
            .filter(|f| f.vendor_ui && pages.is_none_or(|ps| ps.contains(&f.page)) && !profile.supported(f));
        let mut features: Vec<FeatureState> = feats
            .iter()
            .map(|f| {
                let r = cur(f.code);
                let (value, max) = match (f.encoding, r) {
                    (_, None) | (Encoding::Command, _) => (None, None),
                    (Encoding::Range | Encoding::Info, Some(r)) => (Some(r.current), Some(r.max)),
                    (Encoding::Choice, Some(r)) => (Some(r.current), None),
                    (Encoding::LowByte, Some(r)) => (Some(r.current & 0xFF), None),
                    (Encoding::HighByte, Some(r)) => (Some(r.current >> 8), None),
                    (Encoding::Bit { mask } | Encoding::Trigger { mask }, Some(r)) => {
                        (Some(u16::from(r.current & mask != 0)), None)
                    }
                };
                FeatureState {
                    id: f.id,
                    label: f.label,
                    page: f.page,
                    code: f.code,
                    encoding: f.encoding,
                    risk: f.risk,
                    vendor_ui: f.vendor_ui,
                    value,
                    max,
                    options: profile.options(f),
                    supported: true,
                    enabled: features::enabled(f, &context),
                }
            })
            .collect();
        features.extend(greyed.map(|f| FeatureState {
            id: f.id,
            label: f.label,
            page: f.page,
            code: f.code,
            encoding: f.encoding,
            risk: f.risk,
            vendor_ui: true,
            value: None,
            max: None,
            options: Vec::new(),
            supported: false,
            enabled: false,
        }));
        // Catalogue order, so pages can render in DisplayWidget's order.
        let pos = |id: &str| features::FEATURES.iter().position(|f| f.id == id).unwrap_or(usize::MAX);
        features.sort_by_key(|f| pos(f.id));
        MonitorState {
            id: self.id(),
            model: self.model().to_string(),
            line: self.line,
            oled: profile.is_oled(),
            firmware: self.fw,
            bus: self.found.bus,
            connector: self.found.connector.clone(),
            hdr_format: self.hdr_format(),
            context,
            features,
        }
    }

    /// Write one feature. `value` is in the feature's own units (byte for
    /// Low/HighByte, 0/1 for bits). Every write is read back and verified,
    /// except write-only commands. Returns a short description of what was done.
    pub fn apply(&mut self, id: &str, value: u16) -> Result<String> {
        let f = features::by_id(id).with_context(|| format!("unknown feature {id:?}"))?;
        let profile = self.profile();
        if !profile.supported(f) {
            bail!("{} is not supported on this monitor", f.label);
        }
        let opts = profile.options(f);
        if !f.options.is_empty() && !opts.iter().any(|o| o.value == value) {
            bail!("{}: value {value} is not one of this monitor's options", f.label);
        }
        let settle = match f.code {
            0xDC | 0xE2 | 0xE1 | 0xF4 => Duration::from_millis(600),
            _ => Duration::from_millis(60),
        };
        let code = f.code;
        match f.encoding {
            Encoding::Info => bail!("{} is read-only", f.label),
            Encoding::Range => {
                let max = self.get(code)?.max;
                if value > max {
                    bail!("{}: {value} is above the maximum {max}", f.label);
                }
                self.set_verified(code, value, settle)?;
            }
            Encoding::Choice => {
                if is_gameplus(id) && value != 0 {
                    self.clear_other_gameplus(id)?;
                }
                self.set_verified(code, value, settle)?;
            }
            Encoding::LowByte | Encoding::HighByte => {
                let high = matches!(f.encoding, Encoding::HighByte);
                let cur = self.get(code)?.current;
                let word = if high { (cur & 0x00FF) | (value << 8) } else { (cur & 0xFF00) | (value & 0xFF) };
                self.set_verified_with(code, word, settle, |r| {
                    if high { r.current >> 8 == value } else { r.current & 0xFF == value & 0xFF }
                })?;
            }
            Encoding::Bit { mask } => {
                self.set_bits(code, mask, value != 0)?;
            }
            Encoding::Trigger { mask } => {
                if value == 0 {
                    bail!("{} can only be started", f.label);
                }
                // Read first: never write a word built from a failed read.
                let cur = self.get(code).context("reading the register before starting")?;
                if cur.current & mask != 0 {
                    bail!("{} is already running", f.label);
                }
                self.display.set_vcp(code, cur.current | mask)?;
            }
            Encoding::Command => {
                self.display.set_vcp(code, value)?;
            }
        }
        let label = opts.iter().find(|o| o.value == value).map(|o| o.label.to_string()).unwrap_or(value.to_string());
        Ok(format!("{} = {label}", f.label))
    }

    /// GamePlus overlays are exclusive: switch the others off first (as DisplayWidget does).
    fn clear_other_gameplus(&mut self, keep: &str) -> Result<()> {
        for other in ["crosshair", "timer", "fps_counter", "display_alignment"] {
            if other == keep {
                continue;
            }
            let f = features::by_id(other).expect("catalogue entry");
            if !self.profile().supported(f) {
                continue;
            }
            if self.get(f.code)?.current != 0 {
                self.set_verified(f.code, 0, Duration::from_millis(60))?;
            }
        }
        Ok(())
    }
}

fn is_gameplus(id: &str) -> bool {
    matches!(id, "crosshair" | "timer" | "fps_counter" | "display_alignment")
}

/// Bit masks of the packed toggle registers, for diagnostics.
pub fn describe_bits(code: u8, word: u16) -> Vec<&'static str> {
    features::FEATURES
        .iter()
        .filter(|f| f.code == code && bit_mask(f) != 0 && word & bit_mask(f) != 0)
        .map(|f| f.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{self, Fake};
    use serde_json::Value;

    const FIXTURE: &str = include_str!("../../../fixtures/state-pg32ucdm3.json");

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn feature<'a>(st: &'a MonitorState, id: &str) -> &'a FeatureState {
        st.features.iter().find(|f| f.id == id).unwrap_or_else(|| panic!("no feature {id}"))
    }

    /// Error text with its context chain.
    fn err(m: &mut Monitor, id: &str, v: u16) -> String {
        format!("{:#}", m.apply(id, v).expect_err("apply should fail"))
    }

    // Decoding

    #[test]
    fn state_matches_the_real_pg32ucdm3_fixture() {
        // The registers in `testing` are the ones the fixture was read from, so
        // decoding them must give the fixture back (it is the daemon's own output).
        let fake = Fake::pg32ucdm3();
        // The kernel's EDID for DP-3: no Dolby Vision block, so "hdr10".
        fake.set_output_edid("DP-3", &[0u8; 256]);
        let mut m = testing::attach(&fake, Some("DP-3"));
        let want: Value = serde_json::from_str(FIXTURE).unwrap();
        let got = serde_json::to_value(m.state(None)).unwrap();
        for key in ["id", "model", "line", "oled", "firmware", "bus", "connector", "hdr_format", "context"] {
            assert_eq!(got[key], want[key], "{key}");
        }
        let (got, want) = (got["features"].as_array().unwrap(), want["features"].as_array().unwrap());
        for w in want {
            let g = got.iter().find(|g| g["id"] == w["id"]).unwrap_or_else(|| panic!("{} missing", w["id"]));
            assert_eq!(g, w, "{}", w["id"]);
        }
        assert_eq!(got.len(), want.len());
        // Same order too (catalogue order).
        let ids = |v: &[Value]| v.iter().map(|f| f["id"].clone()).collect::<Vec<_>>();
        assert_eq!(ids(got), ids(want));
    }

    #[test]
    fn state_decodes_values_by_encoding() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xFB, 0x0302);
        let st = m.state(None);
        // Range: whole word with its maximum.
        let b = feature(&st, "brightness");
        assert_eq!((b.value, b.max), (Some(100), Some(100)));
        // Choice: whole word, no maximum.
        assert_eq!((feature(&st, "shadow_boost").value, feature(&st, "shadow_boost").max), (Some(3), None));
        // High byte 0x64 of 0x6400 is gamma 2.0, in the high byte only.
        assert_eq!(feature(&st, "gamma").value, Some(0x64));
        // One packed word, two bytes (0xFB: shortcut 1 high, shortcut 2 low).
        assert_eq!(feature(&st, "shortcut_1").value, Some(3));
        assert_eq!(feature(&st, "shortcut_2").value, Some(2));
        // Bits of 0xFD 0x7829: dimming, logo, taskbar... on; uniform brightness off; trigger idle.
        for (id, on) in [("screen_dimming", 1), ("logo_detection", 1), ("uniform_brightness", 0), ("pixel_cleaning", 0), ("vrr", 1)] {
            assert_eq!(feature(&st, id).value, Some(on), "{id}");
        }
        // Commands are write-only: nothing to show.
        assert_eq!((feature(&st, "osd_key").value, feature(&st, "osd_key").max), (None, None));
        // Info keeps current and maximum (FW 103 is the maximum of 0xFF).
        assert_eq!((feature(&st, "firmware").value, feature(&st, "firmware").max), (Some(0), Some(103)));
    }

    #[test]
    fn state_tracks_a_trigger_that_is_running() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xFD, 0x7839);
        let st = m.state(None);
        assert_eq!(feature(&st, "pixel_cleaning").value, Some(1));
        assert_eq!(describe_bits(0xFD, 0x7839).len(), 8);
    }

    #[test]
    fn state_reports_hdr_family_and_sub_mode() {
        let (mut m, fake) = testing::pg32ucdm3();
        // Family 1 (HDR10), sub-mode 2: Gaming HDR.
        fake.set_current(0xE2, 0x0102);
        let st = m.state(None);
        assert!(st.context.hdr);
        let hdr = feature(&st, "hdr_mode");
        assert_eq!(hdr.value, Some(0x0102));
        assert_eq!(hdr.options.iter().find(|o| Some(o.value) == hdr.value).map(|o| o.label), Some("Gaming HDR"));
        // Family 2 (Dolby Vision), sub-mode 7.
        fake.set_current(0xE2, 0x0207);
        let st = m.state(None);
        let hdr = feature(&st, "hdr_mode");
        assert_eq!(hdr.options.iter().find(|o| Some(o.value) == hdr.value).map(|o| o.label), Some("Dolby Vision Gaming"));
        // HDR rules: brightness greyed, the mode row and OLED care stay usable.
        assert!(feature(&st, "hdr_mode").enabled && !feature(&st, "brightness").enabled && feature(&st, "screen_move").enabled);
        // 0 and 0xFE mean no HDR signal.
        for e2 in [0x0000, 0x00FE] {
            fake.set_current(0xE2, e2);
            assert!(!m.state(None).context.hdr, "0x{e2:04X}");
        }
    }

    #[test]
    fn hdr_format_comes_from_the_connectors_edid() {
        let fake = Fake::pg32ucdm3();
        let mut dv = vec![0u8; 256];
        dv[140..143].copy_from_slice(&[0x46, 0xD0, 0x00]);
        fake.set_output_edid("DP-2", &[0u8; 256]);
        fake.set_output_edid("DP-3", &dv);
        let hdr_format = |connector| testing::attach(&fake, connector).hdr_format();
        assert_eq!(hdr_format(Some("DP-3")), Some("dolby_vision"));
        assert_eq!(hdr_format(Some("DP-2")), Some("hdr10"));
        // No connector, or none the kernel knows: nothing to say.
        assert_eq!(hdr_format(None), None);
        assert_eq!(hdr_format(Some("HDMI-A-1")), None);
    }

    #[test]
    fn edid_hdr_format_looks_for_the_dolby_vision_block() {
        let mut edid = vec![0u8; 256];
        assert_eq!(edid_hdr_format(&edid), "hdr10");
        // Vendor-specific video data block: tag 3, length 7, OUI 00-D0-46 (LSB first).
        edid[132..139].copy_from_slice(&[0x67, 0x46, 0xD0, 0x00, 0x01, 0x02, 0x03]);
        assert_eq!(edid_hdr_format(&edid), "dolby_vision");
        // An EDID shorter than the OUI is just HDR10.
        assert_eq!(edid_hdr_format(&[0x46, 0xD0]), "hdr10");
    }

    #[test]
    fn state_reports_srgb_as_0x0a() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xDC, 0x0A);
        let st = m.state(None);
        assert_eq!(st.context.mode, 0x0A);
        let gv = feature(&st, "gamevisual");
        assert_eq!(gv.value, Some(0x0A));
        // This monitor has sRGB calibration: the one sRGB option is 0x0A, not 3.
        let srgb: Vec<_> = gv.options.iter().filter(|o| o.label == "sRGB").map(|o| o.value).collect();
        assert_eq!(srgb, [0x0A]);
        // sRGB greys contrast, colour temperature and Shadow Boost but keeps brightness.
        assert!(feature(&st, "brightness").enabled);
        for id in ["contrast", "color_temp", "shadow_boost", "saturation", "gamma"] {
            assert!(!feature(&st, id).enabled, "{id}");
        }
    }

    #[test]
    fn state_reads_the_mode_from_the_low_byte() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xDC, 0x0105);
        let st = m.state(None);
        assert_eq!(st.context.mode, 5);
        assert_eq!(feature(&st, "gamevisual").value, Some(5));
    }

    #[test]
    fn state_context_follows_power_saving_user_colour_and_blf() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xE1, 1);
        fake.set_current(0x14, 0x0B);
        fake.set_current(0xDC, 5);
        fake.set_current(0xE6, 4);
        let st = m.state(None);
        assert!(st.context.power_saving && st.context.user_color && st.context.blf_max);
        // Power saving leaves brightness and the system pages only.
        assert!(feature(&st, "brightness").enabled && !feature(&st, "crosshair").enabled && feature(&st, "language").enabled);
    }

    #[test]
    fn state_copes_with_unreadable_registers() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.fail_reads(0x10);
        fake.fail_reads(0xDC);
        let st = m.state(None);
        let b = feature(&st, "brightness");
        assert!(b.supported && b.value.is_none() && b.max.is_none());
        // An unreadable mode counts as mode 0.
        assert_eq!(st.context.mode, 0);
    }

    #[test]
    fn state_for_some_pages_reads_only_what_they_need() {
        let (mut m, fake) = testing::pg32ucdm3();
        let st = m.state(Some(&[Page::OledCare]));
        assert!(st.features.iter().all(|f| f.page == Page::OledCare));
        let reads = fake.reads();
        for code in [0xFD, 0xF8, 0xF9, 0xDC, 0xE2, 0xE1, 0x14, 0xE6] {
            assert!(reads.contains(&code), "0x{code:02X}");
        }
        assert!(!reads.contains(&0x10) && !reads.contains(&0x62));
        // Each code once.
        let mut sorted = reads.clone();
        sorted.dedup();
        assert_eq!(sorted.len(), reads.len());
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn unsupported_features_are_listed_greyed() {
        let (mut m, _fake) = testing::pg32ucdm3();
        let st = m.state(None);
        for id in ["overdrive", "ascr", "hue"] {
            let f = feature(&st, id);
            assert!(!f.supported && !f.enabled && f.value.is_none(), "{id}");
        }
        // Catalogue entries Sumi adds on top of the vendor UI are not listed when absent.
        assert!(st.features.iter().all(|f| f.supported || f.vendor_ui));
    }

    #[test]
    fn describe_bits_names_the_set_toggles() {
        assert_eq!(describe_bits(0xFD, 0x0010), ["pixel_cleaning"]);
        assert_eq!(describe_bits(0xFD, 0x0009), ["screen_dimming", "vrr"]);
        assert!(describe_bits(0xFD, 0x0004).is_empty());
        assert_eq!(describe_bits(0xFC, 0x0010), ["auto_input"]);
    }

    // Range

    #[test]
    fn range_above_the_maximum_is_refused_without_writing() {
        let (mut m, fake) = testing::pg32ucdm3();
        assert_eq!(err(&mut m, "brightness", 101), "Brightness: 101 is above the maximum 100");
        assert!(fake.writes().is_empty());
        // The maximum is the live one: blue light filter tops out at 4.
        assert!(err(&mut m, "blue_light", 5).contains("above the maximum 4"));
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn range_value_is_written_and_verified() {
        let (mut m, fake) = testing::pg32ucdm3();
        assert_eq!(m.apply("brightness", 60).unwrap(), "Brightness = 60");
        assert_eq!(fake.writes(), [(0x10, 60)]);
        // Maximum check, then the read-back.
        assert_eq!(fake.reads(), [0x10, 0x10]);
        assert_eq!(fake.current(0x10), 60);
        // The maximum itself and zero are fine.
        m.apply("brightness", 100).unwrap();
        m.apply("brightness", 0).unwrap();
        assert_eq!(fake.writes(), [(0x10, 60), (0x10, 100), (0x10, 0)]);
    }

    #[test]
    fn range_read_back_mismatch_is_an_error_after_the_retries() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.ignore_writes(0x10);
        let e = err(&mut m, "brightness", 60);
        assert!(e.contains("wrote 60, monitor reports 100"), "{e}");
        // One write; the maximum read plus four read-backs.
        assert_eq!(fake.writes(), [(0x10, 60)]);
        assert_eq!(fake.reads().len(), 5);
        assert_eq!(fake.pauses().len(), 5);
    }

    #[test]
    fn apply_checks_support_options_and_read_only_before_any_write() {
        let (mut m, fake) = testing::pg32ucdm3();
        assert!(err(&mut m, "overdrive", 1).contains("not supported on this monitor"));
        assert!(err(&mut m, "hue", 1).contains("not supported"));
        assert!(err(&mut m, "no_such_feature", 1).contains("unknown feature"));
        assert!(err(&mut m, "firmware", 1).contains("read-only"));
        // sRGB is 0x0A on this monitor; plain 3 is not one of its options.
        assert!(err(&mut m, "gamevisual", 3).contains("not one of this monitor's options"));
        assert!(err(&mut m, "timer", 6).contains("not one of this monitor's options"));
        // Old-style crosshairs (1..6) are not offered on the new-style monitors.
        assert!(err(&mut m, "crosshair", 1).contains("not one of this monitor's options"));
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn apply_reports_the_option_label() {
        let (mut m, fake) = testing::pg32ucdm3();
        assert_eq!(m.apply("shadow_boost", 1).unwrap(), "Shadow Boost = Level 1");
        assert_eq!(m.apply("gamevisual", 0x0A).unwrap(), "GameVisual = sRGB");
        assert_eq!(fake.writes(), [(0xE5, 1), (0xDC, 0x0A)]);
    }

    #[test]
    fn slow_settings_get_a_longer_settle_time() {
        let (mut m, fake) = testing::pg32ucdm3();
        for (id, v, settle) in [("brightness", 50, 60), ("gamevisual", 5, 600), ("hdr_mode", 0x0101, 600), ("power_saving", 1, 600), ("pip_mode", 1, 600)] {
            fake.clear_logs();
            m.apply(id, v).unwrap();
            assert_eq!(fake.pauses().first(), Some(&ms(settle)), "{id}");
        }
    }

    #[test]
    fn writes_are_blocked_on_a_read_only_link() {
        let (mut m, fake) = testing::pg32ucdm3();
        m.display.allow_writes(false);
        let e = err(&mut m, "brightness", 50);
        assert!(e.contains("without write access"), "{e}");
        assert!(err(&mut m, "osd_key", 6).contains("without write access"));
        assert!(err(&mut m, "pixel_cleaning", 1).contains("without write access"));
        assert!(fake.writes().is_empty());
    }

    // Choice and the GamePlus overlays

    #[test]
    fn a_gameplus_overlay_switches_the_others_off_first() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xE4, 3); // timer on
        fake.set_current(0xEA, 1); // FPS counter on
        m.apply("crosshair", 7).unwrap();
        // Only the overlays that were on are written, then the new one.
        assert_eq!(fake.writes(), [(0xE4, 0), (0xEA, 0), (0xE3, 7)]);
        assert_eq!((fake.current(0xE4), fake.current(0xEA), fake.current(0xE3)), (0, 0, 7));
        // And back the other way: the crosshair goes when the timer starts.
        fake.clear_logs();
        m.apply("timer", 2).unwrap();
        assert_eq!(fake.writes(), [(0xE3, 0), (0xE4, 2)]);
    }

    #[test]
    fn every_gameplus_overlay_clears_the_other_three() {
        for (id, code, value) in [("crosshair", 0xE3, 7), ("timer", 0xE4, 1), ("fps_counter", 0xEA, 1), ("display_alignment", 0xE7, 1)] {
            let (mut m, fake) = testing::pg32ucdm3();
            let all = [0xE3u8, 0xE4, 0xEA, 0xE7];
            for c in all {
                fake.set_current(c, if c == 0xE3 { 7 } else { 1 });
            }
            m.apply(id, value).unwrap();
            let writes = fake.writes();
            assert_eq!(writes.last(), Some(&(code, value)), "{id}");
            for c in all.into_iter().filter(|&c| c != code) {
                assert_eq!(fake.current(c), 0, "{id}: 0x{c:02X}");
            }
            assert_eq!(writes.len(), 4, "{id}");
        }
    }

    #[test]
    fn turning_an_overlay_off_leaves_the_others_alone() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xE3, 7);
        fake.set_current(0xE4, 3);
        m.apply("crosshair", 0).unwrap();
        assert_eq!(fake.writes(), [(0xE3, 0)]);
        assert_eq!(fake.current(0xE4), 3);
    }

    #[test]
    fn a_failed_overlay_read_aborts_before_the_new_overlay_is_written() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xE4, 3);
        fake.fail_reads(0xEA);
        assert!(m.apply("crosshair", 7).is_err());
        assert!(!fake.writes().contains(&(0xE3, 7)));
    }

    // Low and high byte

    #[test]
    fn low_byte_keeps_the_high_byte() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xDC, 0x0107);
        m.apply("gamevisual", 5).unwrap();
        assert_eq!(fake.writes(), [(0xDC, 0x0105)]);
        // PIP window A is the low byte of 0xF5, window B the high byte.
        fake.set_current(0xF5, 0x1100);
        m.apply("pip_source_a", 0x0F).unwrap();
        assert_eq!(fake.current(0xF5), 0x110F);
    }

    #[test]
    fn high_byte_keeps_the_low_byte() {
        let (mut m, fake) = testing::pg32ucdm3();
        // Gamma 2.0 (0x64) → 2.2 (0x78) in the high byte of 0x72.
        fake.set_current(0x72, 0x6405);
        assert_eq!(m.apply("gamma", 0x78).unwrap(), "Gamma = 2.2");
        assert_eq!(fake.writes(), [(0x72, 0x7805)]);
        fake.set_current(0xF5, 0x000F);
        m.apply("pip_source_b", 0x12).unwrap();
        assert_eq!(fake.current(0xF5), 0x120F);
        // Shortcut 1 is the high byte of 0xFB, shortcut 2 the low byte.
        fake.set_current(0xFB, 0x0102);
        m.apply("shortcut_1", 0x03).unwrap();
        assert_eq!(fake.current(0xFB), 0x0302);
        m.apply("shortcut_2", 0x05).unwrap();
        assert_eq!(fake.current(0xFB), 0x0305);
    }

    #[test]
    fn byte_writes_verify_only_their_byte() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.ignore_writes(0x72);
        let e = err(&mut m, "gamma", 0x78);
        assert!(e.contains("wrote 30720, monitor reports 25600"), "{e}");
        assert_eq!(fake.writes(), [(0x72, 0x7800)]);
    }

    // Packed toggles

    #[test]
    fn a_bit_toggle_changes_only_its_mask() {
        let (mut m, fake) = testing::pg32ucdm3();
        assert_eq!(m.apply("logo_detection", 0).unwrap(), "Logo Detection = 0");
        assert_eq!(fake.writes(), [(0xFD, 0x7809)]);
        m.apply("uniform_brightness", 1).unwrap();
        assert_eq!(fake.writes()[1], (0xFD, 0x7849));
        m.apply("taskbar_detection", 0).unwrap();
        assert_eq!(fake.writes()[2], (0xFD, 0x7049));
        // Any non-zero value switches on.
        m.apply("screen_dimming", 7).unwrap();
        assert_eq!(fake.current(0xFD), 0x7049 | 0x0008);
        // 0xFC: Auto Input Detection only.
        m.apply("auto_input", 0).unwrap();
        assert_eq!(fake.writes().last(), Some(&(0xFC, 0x0801)));
    }

    #[test]
    fn a_failed_read_aborts_a_bit_toggle_without_writing() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.fail_reads(0xFD);
        let e = err(&mut m, "logo_detection", 0);
        assert!(e.contains("before changing it"), "{e}");
        assert!(fake.writes().is_empty());
        // A flaky read that comes good is retried by the transport, not here:
        // one failure is one refusal, and nothing was written for it.
        fake.heal_reads(0xFD);
        fake.fail_reads_times(0xFD, 1);
        assert!(m.apply("logo_detection", 0).is_err());
        assert!(fake.writes().is_empty());
        m.apply("logo_detection", 0).unwrap();
        assert_eq!(fake.writes(), [(0xFD, 0x7809)]);
    }

    #[test]
    fn a_bit_the_monitor_does_not_advertise_is_refused() {
        let (mut m, fake) = testing::pg32ucdm3();
        // The capability string says dimming is supported but the live support mask lacks it.
        fake.set_reg(0xFD, 0x7871, 0x7829);
        let e = err(&mut m, "screen_dimming", 0);
        assert!(e.contains("bit mask 0x0008 not supported (support mask 0x7871)"), "{e}");
        assert!(fake.writes().is_empty());
        // The rest still work.
        m.apply("logo_detection", 0).unwrap();
    }

    fn fd_toggles() -> Vec<&'static str> {
        features::FEATURES.iter().filter(|f| f.code == 0xFD && matches!(f.encoding, Encoding::Bit { .. })).map(|f| f.id).collect()
    }

    #[test]
    fn no_fd_toggle_is_written_while_pixel_cleaning_runs() {
        let toggles = fd_toggles();
        assert!(toggles.len() >= 7, "{toggles:?}");
        for id in toggles {
            for value in [0, 1] {
                // Bit 4 reads 1 while the panel is being cleaned.
                let (mut m, fake) = testing::pg32ucdm3();
                fake.set_current(0xFD, 0x7839);
                let supported = m.profile().supported(features::by_id(id).unwrap());
                let e = err(&mut m, id, value);
                if supported {
                    assert!(e.contains("pixel cleaning is running"), "{id}={value}: {e}");
                }
                assert!(fake.writes().is_empty(), "{id}={value} wrote {:?}", fake.writes());
            }
        }
    }

    #[test]
    fn a_trigger_bit_is_never_part_of_another_toggle() {
        for start in [0x7829u16, 0x0000, 0x7869, 0x6001, 0x0809, 0x7C28] {
            for id in fd_toggles() {
                for value in [0, 1] {
                    let (mut m, fake) = testing::pg32ucdm3();
                    fake.set_current(0xFD, start);
                    if m.apply(id, value).is_ok() {
                        for (code, word) in fake.writes() {
                            assert_eq!((code, word & 0x0010), (0xFD, 0), "{id}={value} from 0x{start:04X} wrote 0x{word:04X}");
                        }
                    }
                }
            }
        }
    }

    // Triggers

    #[test]
    fn pixel_cleaning_starts_by_setting_its_bit() {
        let (mut m, fake) = testing::pg32ucdm3();
        assert_eq!(m.apply("pixel_cleaning", 1).unwrap(), "Pixel Cleaning = 1");
        // The current word with bit 4 added, nothing else, and no read-back
        // (the monitor clears the bit by itself).
        assert_eq!(fake.writes(), [(0xFD, 0x7839)]);
        assert_eq!(fake.reads(), [0xFD]);
    }

    #[test]
    fn pixel_cleaning_refuses_to_start_twice() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.set_current(0xFD, 0x7839);
        assert!(err(&mut m, "pixel_cleaning", 1).contains("is already running"));
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn pixel_cleaning_refuses_when_the_register_cannot_be_read() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.fail_reads(0xFD);
        let e = err(&mut m, "pixel_cleaning", 1);
        assert!(e.contains("reading the register before starting"), "{e}");
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn pixel_cleaning_cannot_be_stopped() {
        let (mut m, fake) = testing::pg32ucdm3();
        assert!(err(&mut m, "pixel_cleaning", 0).contains("can only be started"));
        assert!(fake.writes().is_empty() && fake.reads().is_empty());
    }

    // Write-only commands

    #[test]
    fn commands_are_written_without_a_read_back() {
        let (mut m, fake) = testing::pg32ucdm3();
        // 0xE8, 0xEB and 0xEC answer Get with "unsupported": reading them back would fail.
        for code in [0xE8, 0xEB, 0xEC] {
            assert!(m.get(code).is_err());
        }
        fake.clear_logs();
        m.apply("osd_key", 6).unwrap();
        m.apply("gameplus_position", 3).unwrap();
        m.apply("reset", 0x01).unwrap();
        assert_eq!(fake.writes(), [(0xEB, 6), (0xE8, 3), (0xEC, 1)]);
        assert!(fake.reads().is_empty());
        assert!(fake.pauses().is_empty());
    }

    #[test]
    fn a_command_must_still_be_one_of_the_options() {
        let (mut m, fake) = testing::pg32ucdm3();
        assert!(err(&mut m, "reset", 2).contains("not one of this monitor's options"));
        assert!(err(&mut m, "osd_key", 8).contains("not one of this monitor's options"));
        assert!(err(&mut m, "gameplus_position", 0).contains("not one of this monitor's options"));
        assert!(fake.writes().is_empty());
        assert_eq!(m.apply("reset", 0xFF).unwrap(), "Reset = Reset all settings");
    }
}

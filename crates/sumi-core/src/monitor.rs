//! An attached ASUS monitor: identity, capabilities, VCP scheme, and the
//! verified write path every feature goes through.

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sumi_ddc::{Display, Found};
use sumi_proto::{Capabilities, VcpReply};

use crate::transport::Transport;

/// ASUS product line, from the model-name prefix (DisplayWidget `GetProductLine`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductLine {
    Gaming,
    MainStream,
    Portable,
    ProArt,
    Unknown,
}

impl ProductLine {
    pub fn from_model(model: &str) -> Self {
        let m = model.to_ascii_lowercase();
        if m.contains("_d") {
            return ProductLine::Gaming;
        }
        let p: String = m.chars().take(2).collect();
        match p.as_str() {
            "mb" | "mq" => ProductLine::Portable,
            "vx" | "vp" | "vy" | "vz" | "vu" | "va" | "vt" | "be" => ProductLine::MainStream,
            "pa" | "pq" => ProductLine::ProArt,
            "vg" | "pg" | "xg" => ProductLine::Gaming,
            _ => ProductLine::Unknown,
        }
    }
}

/// Firmware-reported identity (VCP 0xEF / 0xFF).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FirmwareInfo {
    /// ASUS VCP scheme major.minor from 0xEF's maximum (2 = "new VCP").
    pub scheme_major: u8,
    pub scheme_minor: u8,
    pub fw_version: u16,
    pub panel_id: u8,
}

impl FirmwareInfo {
    pub fn new_vcp(&self) -> bool {
        self.scheme_major == 2
    }
}

pub struct Monitor {
    /// The DDC/CI link: the i2c bus in production, an in-memory fake in tests.
    pub display: Box<dyn Transport>,
    pub found: Found,
    pub caps: Capabilities,
    pub line: ProductLine,
    pub fw: FirmwareInfo,
}

/// Stable id for a monitor: model plus bus (serials stay in memory only).
pub fn monitor_id(found: &Found) -> String {
    format!("{}@i2c-{}", found.edid.name.as_deref().unwrap_or("unknown"), found.bus)
}

impl Monitor {
    /// Open, fetch capabilities and firmware identity. Read-only.
    pub fn attach(found: Found) -> Result<Self> {
        Self::attach_cached(found, None)
    }

    /// As `attach`, caching the capability string in `cache_dir` keyed by
    /// model and firmware version (like DisplayWidget), which saves ~1.5 s.
    pub fn attach_cached(found: Found, cache_dir: Option<&Path>) -> Result<Self> {
        let display = Display::open(found.bus)?;
        Self::attach_with(Box::new(display), found, cache_dir)
    }

    /// As `attach_cached`, over any transport (tests pass a fake).
    pub fn attach_with(mut display: Box<dyn Transport>, found: Found, cache_dir: Option<&Path>) -> Result<Self> {
        let ef = display.get_vcp(0xEF).ok();
        let ff = display.get_vcp(0xFF).ok();
        let fw = FirmwareInfo {
            scheme_major: ef.map(|r| (r.max >> 8) as u8).unwrap_or(0),
            scheme_minor: ef.map(|r| r.max as u8).unwrap_or(0),
            fw_version: ff.map(|r| r.max).unwrap_or(0),
            panel_id: ff.map(|r| r.current as u8).unwrap_or(0),
        };
        let edid_model = found.edid.name.clone().unwrap_or_default();
        let cache = cache_dir.filter(|_| ff.is_some() && !edid_model.is_empty()).map(|d| {
            let safe: String = edid_model.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            d.join(format!("caps-{safe}-fw{}.txt", fw.fw_version))
        });
        // An empty or cut-off string is never cached: it would hide every feature until deleted.
        let has_features = |s: &str| !sumi_proto::caps::parse(s).vcp.is_empty();
        let raw = match cache.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).filter(|s| has_features(s)) {
            Some(s) => s,
            None => {
                let s = display.capabilities_raw().context("reading capability string")?;
                if let Some(p) = cache.as_ref().filter(|_| has_features(&s)) {
                    let _ = p.parent().map(std::fs::create_dir_all);
                    let _ = std::fs::write(p, &s);
                }
                s
            }
        };
        let caps = sumi_proto::caps::parse(&raw);
        let model = caps.model().map(str::to_string).unwrap_or(edid_model);
        Ok(Monitor { display, found, caps, line: ProductLine::from_model(&model), fw })
    }

    pub fn id(&self) -> String {
        monitor_id(&self.found)
    }

    pub fn model(&self) -> &str {
        self.caps.model().or(self.found.edid.name.as_deref()).unwrap_or("unknown")
    }

    pub fn get(&mut self, code: u8) -> Result<VcpReply> {
        Ok(self.display.get_vcp(code)?)
    }

    /// Write `value` and read it back until it matches (some settings take a
    /// moment to land). `settle` is extra time to wait before the first read.
    pub fn set_verified(&mut self, code: u8, value: u16, settle: Duration) -> Result<VcpReply> {
        self.set_verified_with(code, value, settle, |r| r.current == value)
    }

    /// Write and verify with a custom acceptance test on the read-back.
    pub fn set_verified_with(
        &mut self,
        code: u8,
        value: u16,
        settle: Duration,
        ok: impl Fn(&VcpReply) -> bool,
    ) -> Result<VcpReply> {
        self.display.set_vcp(code, value).with_context(|| format!("writing VCP 0x{code:02X}={value}"))?;
        self.display.pause(settle);
        let mut last = None;
        for attempt in 0..4 {
            match self.display.get_vcp(code) {
                Ok(r) if ok(&r) => return Ok(r),
                Ok(r) => last = Some(r),
                Err(e) if attempt == 3 => return Err(e).context(format!("reading back VCP 0x{code:02X}")),
                Err(_) => {}
            }
            self.display.pause(Duration::from_millis(100 * (attempt + 1)));
        }
        let got = last.map(|r| r.current.to_string()).unwrap_or_else(|| "nothing".into());
        bail!("VCP 0x{code:02X}: wrote {value}, monitor reports {got}")
    }

    /// Read a packed register (0xFC/0xFD style) as the base of a
    /// read-modify-write. A failed read aborts (never write a word built from
    /// nothing), a running trigger (pixel cleaning) refuses the write, and the
    /// returned word has the trigger bits cleared so they are never replayed.
    pub fn read_packed(&mut self, code: u8) -> Result<VcpReply> {
        let mut cur = self.get(code).with_context(|| format!("reading VCP 0x{code:02X} before changing it"))?;
        let triggers = crate::features::trigger_bits(code);
        if cur.current & triggers != 0 {
            bail!("VCP 0x{code:02X}: pixel cleaning is running; try again when it has finished");
        }
        cur.current &= !triggers;
        Ok(cur)
    }

    /// Read-modify-write one bit group of a packed register (0xFC/0xFD style).
    pub fn set_bits(&mut self, code: u8, mask: u16, on: bool) -> Result<VcpReply> {
        if mask & crate::features::trigger_bits(code) != 0 {
            bail!("VCP 0x{code:02X}: bit mask 0x{mask:04X} is an action trigger, not a toggle");
        }
        let cur = self.read_packed(code)?;
        if cur.max & mask == 0 {
            bail!("VCP 0x{code:02X}: bit mask 0x{mask:04X} not supported (support mask 0x{:04X})", cur.max);
        }
        let want = if on { cur.current | mask } else { cur.current & !mask };
        self.set_verified_with(code, want, Duration::from_millis(100), |r| (r.current & mask == mask) == on)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{self, Fake};

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn product_lines() {
        assert_eq!(ProductLine::from_model("PG32UCDM3"), ProductLine::Gaming);
        assert_eq!(ProductLine::from_model("XG27AQDMG"), ProductLine::Gaming);
        assert_eq!(ProductLine::from_model("PA32UCDM"), ProductLine::ProArt);
        assert_eq!(ProductLine::from_model("MB16AHG"), ProductLine::Portable);
        assert_eq!(ProductLine::from_model("VG27AQ_D"), ProductLine::Gaming);
        assert_eq!(ProductLine::from_model("VA24DQ"), ProductLine::MainStream);
        assert_eq!(ProductLine::from_model("PQ27UCT"), ProductLine::ProArt);
        assert_eq!(ProductLine::from_model("ROG"), ProductLine::Unknown);
        assert_eq!(ProductLine::from_model(""), ProductLine::Unknown);
    }

    #[test]
    fn attach_derives_identity_and_firmware() {
        let fake = Fake::pg32ucdm3();
        let m = testing::attach(&fake, Some("DP-3"));
        assert_eq!((m.fw.scheme_major, m.fw.scheme_minor), (2, 18));
        assert!(m.fw.new_vcp());
        assert_eq!((m.fw.fw_version, m.fw.panel_id), (103, 0));
        assert_eq!(m.line, ProductLine::Gaming);
        assert_eq!(m.model(), "PG32UCDM3");
        assert_eq!(m.id(), "PG32UCDM3@i2c-5");
        assert_eq!(m.found.connector.as_deref(), Some("DP-3"));
        assert!(m.profile().is_oled());
        // Attaching is read-only: identity registers only.
        assert!(fake.writes().is_empty());
        assert_eq!(fake.reads(), [0xEF, 0xFF]);
        assert_eq!(fake.caps_reads(), 1);
    }

    #[test]
    fn attach_survives_unreadable_identity() {
        let fake = Fake::pg32ucdm3();
        fake.fail_reads(0xEF);
        fake.fail_reads(0xFF);
        let m = testing::attach(&fake, None);
        assert_eq!((m.fw.scheme_major, m.fw.fw_version), (0, 0));
        // Not the new scheme: the catalogue falls back to the generic controls.
        assert!(!m.fw.new_vcp());
        assert!(m.profile().supported(crate::features::by_id("brightness").unwrap()));
        assert!(!m.profile().supported(crate::features::by_id("pixel_cleaning").unwrap()));
    }

    #[test]
    fn model_comes_from_capabilities_before_edid() {
        let fake = Fake::pg32ucdm3();
        let mut found = testing::found(None);
        found.edid.name = Some("SOMETHING ELSE".into());
        let m = Monitor::attach_with(Box::new(fake), found, None).unwrap();
        assert_eq!(m.model(), "PG32UCDM3");
        assert_eq!(m.line, ProductLine::Gaming);
    }

    #[test]
    fn capability_string_is_cached_by_model_and_firmware() {
        let dir = std::env::temp_dir().join(format!("sumi-test-caps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fake = Fake::pg32ucdm3();
        let first = Monitor::attach_with(Box::new(fake.clone()), testing::found(None), Some(&dir)).unwrap();
        assert_eq!(fake.caps_reads(), 1);
        assert!(dir.join("caps-PG32UCDM3-fw103.txt").is_file());
        let second = Monitor::attach_with(Box::new(fake.clone()), testing::found(None), Some(&dir)).unwrap();
        assert_eq!(fake.caps_reads(), 1, "second attach should use the cache");
        assert_eq!(first.caps, second.caps);
        // A different firmware is a different cache entry.
        fake.set_reg(0xFF, 104, 0);
        Monitor::attach_with(Box::new(fake.clone()), testing::found(None), Some(&dir)).unwrap();
        assert_eq!(fake.caps_reads(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_empty_capability_string_is_not_cached() {
        let dir = std::env::temp_dir().join(format!("sumi-test-emptycaps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fake = Fake::pg32ucdm3();
        fake.set_caps("");
        let m = Monitor::attach_with(Box::new(fake.clone()), testing::found(None), Some(&dir)).unwrap();
        assert!(m.caps.vcp.is_empty());
        assert!(!dir.join("caps-PG32UCDM3-fw103.txt").exists());
        // A cache file that is already empty (or junk) is ignored, and replaced by a good read.
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("caps-PG32UCDM3-fw103.txt"), "").unwrap();
        fake.set_caps(testing::PG32UCDM3_CAPS);
        let m = Monitor::attach_with(Box::new(fake.clone()), testing::found(None), Some(&dir)).unwrap();
        assert!(m.caps.supports(0xDC));
        assert_eq!(std::fs::read_to_string(dir.join("caps-PG32UCDM3-fw103.txt")).unwrap(), testing::PG32UCDM3_CAPS);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_cache_without_a_firmware_version() {
        let dir = std::env::temp_dir().join(format!("sumi-test-nocaps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fake = Fake::pg32ucdm3();
        fake.fail_reads(0xFF);
        Monitor::attach_with(Box::new(fake.clone()), testing::found(None), Some(&dir)).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn set_verified_writes_then_reads_back() {
        let (mut m, fake) = testing::pg32ucdm3();
        let r = m.set_verified(0x10, 60, ms(60)).unwrap();
        assert_eq!(r.current, 60);
        assert_eq!(fake.writes(), [(0x10, 60)]);
        assert_eq!(fake.reads(), [0x10]);
        assert_eq!(fake.pauses(), [ms(60)]);
    }

    #[test]
    fn set_verified_retries_a_flaky_read_back() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.fail_reads_times(0x10, 2);
        assert_eq!(m.set_verified(0x10, 60, ms(0)).unwrap().current, 60);
        // One write; two failed reads and the good one, with a growing wait between.
        assert_eq!(fake.writes(), [(0x10, 60)]);
        assert_eq!(fake.reads(), [0x10, 0x10, 0x10]);
        assert_eq!(fake.pauses(), [ms(0), ms(100), ms(200)]);
    }

    #[test]
    fn set_verified_gives_up_on_unreadable_register() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.fail_reads(0x10);
        let e = format!("{:#}", m.set_verified(0x10, 60, ms(0)).unwrap_err());
        assert!(e.contains("reading back VCP 0x10"), "{e}");
        assert_eq!(fake.writes(), [(0x10, 60)]);
        assert_eq!(fake.reads().len(), 4);
    }

    #[test]
    fn set_verified_reports_the_value_the_monitor_kept() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.ignore_writes(0x10);
        let e = m.set_verified(0x10, 60, ms(0)).unwrap_err().to_string();
        assert_eq!(e, "VCP 0x10: wrote 60, monitor reports 100");
        // Written once, read four times.
        assert_eq!(fake.writes(), [(0x10, 60)]);
        assert_eq!(fake.reads(), [0x10; 4]);
        assert_eq!(fake.pauses(), [ms(0), ms(100), ms(200), ms(300), ms(400)]);
    }

    #[test]
    fn set_verified_with_uses_the_custom_test() {
        let (mut m, fake) = testing::pg32ucdm3();
        // Only the high byte has to land.
        let r = m.set_verified_with(0x72, 0x7800, ms(0), |r| r.current >> 8 == 0x78).unwrap();
        assert_eq!(r.current, 0x7800);
        fake.ignore_writes(0x72);
        assert!(m.set_verified_with(0x72, 0x8C00, ms(0), |r| r.current >> 8 == 0x8C).is_err());
    }

    #[test]
    fn writes_are_refused_until_allowed() {
        let fake = Fake::pg32ucdm3();
        let mut m = testing::attach(&fake, None);
        m.display.allow_writes(false);
        let e = format!("{:#}", m.set_verified(0x10, 60, ms(0)).unwrap_err());
        assert!(e.contains("writing VCP 0x10=60") && e.contains("without write access"), "{e}");
        assert!(fake.writes().is_empty());
        m.display.allow_writes(true);
        m.set_verified(0x10, 60, ms(0)).unwrap();
    }

    #[test]
    fn read_packed_refuses_a_running_trigger() {
        let (mut m, fake) = testing::pg32ucdm3();
        // Idle: the word comes back as read.
        assert_eq!(m.read_packed(0xFD).unwrap().current, 0x7829);
        fake.set_current(0xFD, 0x7839);
        let e = m.read_packed(0xFD).unwrap_err().to_string();
        assert!(e.contains("pixel cleaning is running"), "{e}");
        // 0xFC has no trigger bit: any value is a fine base.
        fake.set_current(0xFC, 0x08FF);
        assert_eq!(m.read_packed(0xFC).unwrap().current, 0x08FF);
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn read_packed_never_builds_a_word_from_a_failed_read() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.fail_reads(0xFD);
        let e = format!("{:#}", m.read_packed(0xFD).unwrap_err());
        assert!(e.contains("reading VCP 0xFD before changing it"), "{e}");
    }

    #[test]
    fn set_bits_changes_only_its_mask() {
        let (mut m, fake) = testing::pg32ucdm3();
        m.set_bits(0xFD, 0x0020, false).unwrap();
        assert_eq!(fake.writes(), [(0xFD, 0x7809)]);
        m.set_bits(0xFD, 0x0040, true).unwrap();
        assert_eq!(fake.writes()[1], (0xFD, 0x7849));
        // Already in the wanted state: still one verified write, same word.
        m.set_bits(0xFD, 0x0040, true).unwrap();
        assert_eq!(fake.writes()[2], (0xFD, 0x7849));
    }

    #[test]
    fn set_bits_refuses_unsupported_masks_and_failed_reads() {
        let (mut m, fake) = testing::pg32ucdm3();
        // ASCR (0x0002) is not in 0xFD's support mask 0x7879.
        let e = m.set_bits(0xFD, 0x0002, true).unwrap_err().to_string();
        assert!(e.contains("bit mask 0x0002 not supported (support mask 0x7879)"), "{e}");
        fake.fail_reads(0xFD);
        assert!(m.set_bits(0xFD, 0x0020, true).is_err());
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn set_bits_never_toggles_a_trigger() {
        let (mut m, fake) = testing::pg32ucdm3();
        for on in [true, false] {
            let e = m.set_bits(0xFD, 0x0010, on).unwrap_err().to_string();
            assert!(e.contains("action trigger"), "{e}");
            // A mask that merely includes the trigger is refused too.
            assert!(m.set_bits(0xFD, 0x0030, on).is_err());
        }
        assert!(fake.writes().is_empty());
        // 0xFC bit 4 is Auto Input Detection, a plain toggle there.
        m.set_bits(0xFC, 0x0010, false).unwrap();
        assert_eq!(fake.writes(), [(0xFC, 0x0801)]);
    }

    #[test]
    fn set_bits_reports_a_bit_the_monitor_did_not_take() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.ignore_writes(0xFD);
        assert!(m.set_bits(0xFD, 0x0020, false).is_err());
        assert_eq!(fake.writes().len(), 1);
    }
}

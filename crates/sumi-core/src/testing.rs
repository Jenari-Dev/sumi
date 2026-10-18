//! An in-memory monitor for tests: a register map behind the `Transport`
//! trait, a log of every read and write, and switches for the ways a real
//! monitor misbehaves. Nothing here opens a bus.
//!
//! ```text
//! let (mut m, fake) = testing::pg32ucdm3();
//! m.apply("brightness", 50)?;
//! assert_eq!(fake.writes(), [(0x10, 50)]);
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use sumi_ddc::{DdcError, Found};
use sumi_proto::{Edid, FrameError, VcpReply, VcpType};

use crate::monitor::Monitor;
use crate::transport::Transport;

/// The PG32UCDM3's real capability string.
pub const PG32UCDM3_CAPS: &str = include_str!("../../../fixtures/pg32ucdm3-capabilities.txt");

/// A PG32UCDM3 at idle (FPS mode, FW 103, VCP scheme 2.18), as read from the
/// real monitor: (code, maximum, current).
pub const PG32UCDM3_REGS: &[(u8, u16, u16)] = &[
    (0x10, 100, 100),
    (0x12, 100, 80),
    (0x14, 0x0B, 0x05),
    (0x16, 100, 100),
    (0x18, 100, 100),
    (0x1A, 100, 100),
    (0x59, 100, 50),
    (0x5A, 100, 50),
    (0x5B, 100, 50),
    (0x5C, 100, 50),
    (0x5D, 100, 50),
    (0x5E, 100, 50),
    (0x5F, 5, 0),
    (0x60, 0x1A, 0x0F),
    (0x62, 100, 50),
    (0x72, 0xA000, 0x6400),
    (0x87, 100, 50),
    (0x8A, 100, 50),
    (0x8D, 2, 2),
    (0xAE, 0xFFFF, 24000),
    (0xC0, 0, 3339),
    (0xCC, 0x27, 2),
    (0xDC, 0x0A, 7),
    (0xE1, 1, 0),
    (0xE2, 0x0208, 0x0000),
    (0xE3, 0x0F, 0),
    (0xE4, 5, 0),
    (0xE5, 4, 3),
    (0xE6, 4, 0),
    (0xE7, 1, 0),
    (0xEA, 2, 0),
    (0xEE, 0, 0),
    (0xEF, 0x0212, 0x00B1),
    (0xF3, 4, 2),
    (0xF4, 5, 0),
    (0xF5, 0x1A1A, 0),
    (0xF6, 0x0A0A, 0),
    (0xF8, 8, 8),
    (0xF9, 3, 2),
    (0xFB, 0x0C0C, 0x0102),
    (0xFC, 0x089F, 0x0811),
    (0xFD, 0x7879, 0x7829),
    (0xFF, 103, 0),
];

#[derive(Default)]
struct Inner {
    regs: BTreeMap<u8, (u16, u16)>,
    caps: String,
    writes_allowed: bool,
    writes: Vec<(u8, u16)>,
    reads: Vec<u8>,
    pauses: Vec<Duration>,
    /// Code → (reads that still succeed, then failures left; `None` = every read fails).
    failing: BTreeMap<u8, (u32, Option<u32>)>,
    ignoring: BTreeSet<u8>,
    caps_reads: u32,
    outputs: Vec<(String, Vec<u8>)>,
}

/// A fake DDC/CI link. Clones share one state, so a test keeps a handle while
/// the `Monitor` owns another.
#[derive(Clone, Default)]
pub struct Fake(Arc<Mutex<Inner>>);

impl Fake {
    /// No registers, no capability string; writes refused until allowed (as with `Display`).
    pub fn empty() -> Self {
        Fake::default()
    }

    /// A PG32UCDM3 at idle with writes allowed.
    pub fn pg32ucdm3() -> Self {
        let f = Fake::empty();
        for &(code, max, current) in PG32UCDM3_REGS {
            f.set_reg(code, max, current);
        }
        {
            let mut i = f.inner();
            i.caps = PG32UCDM3_CAPS.to_string();
            i.writes_allowed = true;
        }
        // Write-only codes (0xE8, 0xEB, 0xEC…) have no register, so Get answers
        // "unsupported" as on the real monitor.
        f
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap()
    }

    /// Define (or redefine) a register.
    pub fn set_reg(&self, code: u8, max: u16, current: u16) {
        self.inner().regs.insert(code, (max, current));
    }

    /// Change a register's current value, as the OSD would.
    pub fn set_current(&self, code: u8, current: u16) {
        self.inner().regs.get_mut(&code).unwrap_or_else(|| panic!("no register 0x{code:02X}")).1 = current;
    }

    /// A register's current value.
    pub fn current(&self, code: u8) -> u16 {
        self.inner().regs.get(&code).unwrap_or_else(|| panic!("no register 0x{code:02X}")).1
    }

    /// Every Set that reached the link, in order.
    pub fn writes(&self) -> Vec<(u8, u16)> {
        self.inner().writes.clone()
    }

    /// Every Get, in order.
    pub fn reads(&self) -> Vec<u8> {
        self.inner().reads.clone()
    }

    /// Forget the logs (not the registers), e.g. after setting a scene up.
    pub fn clear_logs(&self) {
        let mut i = self.inner();
        i.writes.clear();
        i.reads.clear();
        i.pauses.clear();
    }

    /// Every pause a caller asked for (nothing actually sleeps).
    pub fn pauses(&self) -> Vec<Duration> {
        self.inner().pauses.clone()
    }

    /// Reads of `code` fail from now on.
    pub fn fail_reads(&self, code: u8) {
        self.inner().failing.insert(code, (0, None));
    }

    /// The next `n` reads of `code` fail, then it answers again.
    pub fn fail_reads_times(&self, code: u8, n: u32) {
        self.inner().failing.insert(code, (0, Some(n)));
    }

    /// The next `ok` reads of `code` work, then every read fails.
    pub fn fail_reads_after(&self, code: u8, ok: u32) {
        self.inner().failing.insert(code, (ok, None));
    }

    /// Reads of `code` work again.
    pub fn heal_reads(&self, code: u8) {
        self.inner().failing.remove(&code);
    }

    /// The monitor accepts writes to `code` on the wire but does not change it.
    pub fn ignore_writes(&self, code: u8) {
        self.inner().ignoring.insert(code);
    }

    /// Writes to `code` take effect again.
    pub fn accept_writes(&self, code: u8) {
        self.inner().ignoring.remove(&code);
    }

    /// The kernel's EDID for a display output (none are known by default).
    pub fn set_output_edid(&self, connector: &str, edid: &[u8]) {
        self.inner().outputs.push((connector.to_string(), edid.to_vec()));
    }

    /// Replace the capability string the monitor reports.
    pub fn set_caps(&self, caps: &str) {
        self.inner().caps = caps.to_string();
    }

    /// How many times the capability string was fetched.
    pub fn caps_reads(&self) -> u32 {
        self.inner().caps_reads
    }
}

impl Transport for Fake {
    fn get_vcp(&mut self, code: u8) -> sumi_ddc::Result<VcpReply> {
        let mut i = self.inner();
        i.reads.push(code);
        let frame = |source| DdcError::Frame { code, source };
        if let Some((ok, fails)) = i.failing.get_mut(&code) {
            if *ok > 0 {
                *ok -= 1;
            } else {
                match fails {
                    None => return Err(frame(FrameError::Null)),
                    Some(n) if *n > 0 => {
                        *n -= 1;
                        return Err(frame(FrameError::Null));
                    }
                    Some(_) => {}
                }
            }
        }
        match i.regs.get(&code) {
            Some(&(max, current)) => Ok(VcpReply { code, kind: VcpType::SetParameter, max, current }),
            None => Err(frame(FrameError::Unsupported(code))),
        }
    }

    fn set_vcp(&mut self, code: u8, value: u16) -> sumi_ddc::Result<()> {
        let mut i = self.inner();
        if !i.writes_allowed {
            return Err(DdcError::WriteBlocked(code));
        }
        i.writes.push((code, value));
        if !i.ignoring.contains(&code) {
            if let Some(reg) = i.regs.get_mut(&code) {
                reg.1 = value;
            }
        }
        Ok(())
    }

    fn capabilities_raw(&mut self) -> sumi_ddc::Result<String> {
        let mut i = self.inner();
        i.caps_reads += 1;
        Ok(i.caps.clone())
    }

    fn allow_writes(&mut self, yes: bool) {
        self.inner().writes_allowed = yes;
    }

    fn output_edids(&self) -> Vec<(String, Vec<u8>)> {
        self.inner().outputs.clone()
    }

    fn pause(&mut self, d: Duration) {
        self.inner().pauses.push(d);
    }
}

/// Where the fake PG32UCDM3 sits (i2c bus 5). `connector` is `None` unless a
/// test needs one (`Fake::set_output_edid` gives it an EDID).
pub fn found(connector: Option<&str>) -> Found {
    found_on(5, connector)
}

/// As `found`, on another bus (a second monitor gets another id).
pub fn found_on(bus: u32, connector: Option<&str>) -> Found {
    Found {
        bus,
        path: PathBuf::from("fake"),
        adapter: "fake".into(),
        edid: Edid { manufacturer: "AUS".into(), product_code: 0x2000, name: Some("PG32UCDM3".into()), serial: None, year: 2024, week: 10 },
        connector: connector.map(str::to_string),
    }
}

/// Attach a monitor over `fake` the way `Monitor::attach` does (identity,
/// firmware and capabilities all read through the link).
pub fn attach(fake: &Fake, connector: Option<&str>) -> Monitor {
    attach_on(fake, 5, connector)
}

/// As `attach`, on another bus.
pub fn attach_on(fake: &Fake, bus: u32, connector: Option<&str>) -> Monitor {
    Monitor::attach_with(Box::new(fake.clone()), found_on(bus, connector), None).expect("attach over the fake")
}

/// A PG32UCDM3 at idle on a fake link with writes allowed, and a handle on the link.
pub fn pg32ucdm3() -> (Monitor, Fake) {
    let fake = Fake::pg32ucdm3();
    let m = attach(&fake, None);
    fake.clear_logs();
    (m, fake)
}
